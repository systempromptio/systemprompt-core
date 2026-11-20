//! [`McpHttpExtension`] (`mcp-http`): the MCP registry route set.
//!
//! It contributes one route set, `GET /api/v1/mcp/registry`,
//! behind the public gate with a public request context, in the
//! MCP-and-stream role group under the `mcp_registry` rate-limit scope and
//! budget. Its shape equals the reference composition's conferral for
//! `mcp-http` at its base, which pins the set to that one route. The handler
//! reads the MCP server registry the `mcp` member provides, which this
//! member `requires`.
//!
//! The MCP executions and proxy routes under `/api/v1/mcp`, the MCP OAuth
//! discovery documents, the MCP child host and `admin mcp` are the kernel's.
//!
//! | profile disables | registry route |
//! |---|---|
//! | `mcp-http` | 404 |
//! | `mcp` | boot refused (unmet `Arc<dyn McpRegistryProvider>`) |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_extension::reserved::CoreId;
use systemprompt_traits::McpRegistryProvider;

use crate::http::registry_routes;

pub const MCP_HTTP_EXTENSION: &str = CoreId::McpHttp.id();

/// The member that contributes the MCP registry route set.
#[derive(Debug, Clone, Copy, Default)]
pub struct McpHttpExtension;

impl Extension for McpHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: MCP_HTTP_EXTENSION,
            name: "MCP HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.requires::<Arc<dyn McpRegistryProvider>>()
            .routes(registry_routes());
        Ok(())
    }
}
