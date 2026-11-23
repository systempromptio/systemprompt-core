//! The generic MCP host: one rmcp server for any set of declared toolsets.
//!
//! [`ToolsetServer`] absorbs what every hand-written MCP server binary
//! repeated — server info and icons, RBAC and the governance hook on every
//! request, an access-audit row per outcome, one `ToolExecution` —
//! and adds per-caller tool filtering from the YAML's `access:` blocks.
//! [`crate::toolsets::serve`] builds the deployment's toolsets from the built
//! toolset slot and binds the
//! listener.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catalog;
mod gate;
mod server;
mod wire;

pub use catalog::{HostedCatalog, HostedTool, ToolsetHostError, resolve_tool_access};
pub use server::{ToolsetAuthz, ToolsetServer, ToolsetServerParts};
pub use wire::{build_tool_list_result, client_profile_from_peer, default_server_icons};

pub async fn serve_router(
    listener: tokio::net::TcpListener,
    router: axum::Router,
) -> std::io::Result<()> {
    axum::serve(listener, router).await
}
