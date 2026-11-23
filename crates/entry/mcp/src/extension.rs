//! [`McpHttpExtension`] (`mcp-http`): the MCP registry route set and the
//! `plugins mcp` commands.
//!
//! With `http` it contributes one route set, `GET /api/v1/mcp/registry`,
//! behind the public gate with a public request context, in the
//! MCP-and-stream role group under the `mcp_registry` rate-limit scope and
//! budget. Its shape equals the reference composition's conferral for
//! `mcp-http` at its base, which pins the set to that one route. The handler
//! reads the MCP server registry the `mcp` member provides.
//!
//! With `cli` it contributes the `plugins mcp` group under the kernel's
//! `plugins` group, requesting the `services-tree` grant (for the API's
//! external URL) and the `caller-session` grant its reserved path lists. The
//! commands read the registry and the read-only `Arc<McpServiceObserver>` the
//! `mcp` member provides; none of them can start, stop or signal a server,
//! and `tools` and `call` reach a server only through the API's MCP proxy
//! with the caller's own session. The member `requires` each value it reads.
//!
//! The MCP executions and proxy routes under `/api/v1/mcp`, the MCP OAuth
//! discovery documents, the MCP child host and `admin mcp` are the kernel's.
//!
//! | profile disables | registry route | `plugins mcp` |
//! |---|---|---|
//! | `mcp-http` | 404 | refused at dispatch |
//! | `mcp` | boot refused (unmet requirements) | boot refused |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;
use systemprompt_extension::reserved::CoreId;

pub const MCP_HTTP_EXTENSION: &str = CoreId::McpHttp.id();

/// The member that contributes the MCP registry route set and commands.
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

    #[cfg_attr(
        not(any(feature = "http", feature = "cli")),
        expect(
            unused_variables,
            reason = "the member contributes only with the http or cli feature"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(any(feature = "http", feature = "cli"))]
        c.requires::<std::sync::Arc<dyn systemprompt_traits::McpRegistryProvider>>();
        #[cfg(feature = "http")]
        c.routes(crate::http::registry_routes());
        #[cfg(feature = "cli")]
        commands::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "cli")]
mod commands {
    use std::sync::Arc;

    use systemprompt_extension::cli::{CommandClaim, CommandDecl, KernelGrant, parents};
    use systemprompt_extension::prelude::*;
    use systemprompt_mcp::services::McpServiceObserver;

    use crate::cli::McpCommands;

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<Arc<McpServiceObserver>>().command(
            CommandDecl::group_under::<parents::Plugins, McpCommands>()
                .requesting(&[KernelGrant::ServicesTree, KernelGrant::CallerSession])
                .claiming(&[CommandClaim::PreservesData]),
        );
    }
}
