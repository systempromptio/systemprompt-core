//! The MCP HTTP surface.
//!
//! [`registry_routes`] is the one route set
//! [`crate::extension::McpHttpExtension`] contributes; [`McpHttpSurface`] is
//! the [`McpSurface`] the API mounts the tool-execution lookup from under its
//! own declaration (authenticated, a user context, the `mcp` rate scope and
//! budget).
//!
//! `GET /api/v1/mcp/registry` lists the enabled MCP servers with the endpoint
//! a client connects to, behind the public gate with a public request
//! context, under the `mcp_registry` rate-limit scope and budget with the
//! default body limit, in the MCP-and-stream role group. The handler reads the
//! MCP server registry the `mcp` member provides
//! (`Arc<dyn McpRegistryProvider>`).
//!
//! The base is nested in the kernel's `/api/v1/mcp` namespace, where the MCP
//! proxy serves `/{service_name}/...` and receives callers' bearer tokens.
//! The conferral pins the set to `GET /` and the reserved entry holds
//! `/api/v1/mcp/registry` exactly, so the set answers that one path and no
//! other path under `/api/v1/mcp`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod executions;
pub mod registry;

use std::sync::Arc;

use axum::http::Method;
use systemprompt_extension::router::{
    ApiBase, BasePath, BodyLimit, McpRoutes, McpSurface, RouteLimits, RouteSet, RouteSpec,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, FromState, MissingDependency, RateBudget, RateLimit, Resolved,
    RouteGroup,
};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_traits::McpRegistryProvider;

pub use executions::{ExecutionsState, ToolExecutionResponse, executions_router};
pub use registry::{McpRegistryServer, handle_mcp_registry};

const SPEC: RouteSpec = RouteSpec::new(
    AuthzPolicy::public(),
    ContextKind::Public,
    RouteLimits {
        rate: RateLimit::new("mcp_registry", RateBudget::McpRegistry),
        body: BodyLimit::Default,
    },
);

/// Router-scoped state of the MCP registry route.
#[derive(Clone)]
pub struct McpRegistryState {
    registry: Arc<dyn McpRegistryProvider>,
}

impl std::fmt::Debug for McpRegistryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpRegistryState").finish_non_exhaustive()
    }
}

impl McpRegistryState {
    #[must_use]
    pub fn new(registry: Arc<dyn McpRegistryProvider>) -> Self {
        Self { registry }
    }
}

impl FromState for McpRegistryState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self::new(r.get::<Arc<dyn McpRegistryProvider>>()?))
    }
}

#[must_use]
pub fn registry_routes() -> RouteSet {
    RouteSet::at::<McpRegistryState>(
        BasePath::Api(ApiBase::McpRegistry),
        RouteGroup::McpAndStream,
    )
    .route(Method::GET, "/", handle_mcp_registry, SPEC)
    .build()
}

/// The repositories the `mcp-http` routes read, built in the member's
/// provide fn: the tool-execution ledger.
#[derive(Debug, Clone)]
pub struct McpHttpRepositories {
    pub tool_usage: Arc<ToolUsageRepository>,
}

/// The `mcp-http` member's MCP surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct McpHttpSurface;

impl McpSurface for McpHttpSurface {
    fn build(&self, resolved: &Resolved<'_>) -> Result<McpRoutes, MissingDependency> {
        let repositories = resolved.get::<Arc<McpHttpRepositories>>()?;
        Ok(McpRoutes {
            executions: executions_router(Arc::clone(&repositories.tool_usage)),
        })
    }
}
