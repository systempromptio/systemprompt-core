//! Reverse proxy from the API gateway to local MCP and agent backends.
//!
//! [`ProxyService`] is the entry point; the `auth` submodule is the single
//! authorization boundary for proxied `/api/v1/mcp/*` and `/api/v1/agents/*`
//! traffic, emitting RFC 9728 OAuth challenges on unauthenticated requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
mod backend;
mod backend_error;
mod client;
mod errors;
pub mod resolver;
pub mod service;
mod sse;

pub use auth::OAuthChallenge;
pub use errors::{ProxyError, ResponseBuildError};
pub use service::{ProxyKind, ProxyService, ProxyTarget};

use std::sync::Arc;

use systemprompt_runtime::AppContext;
use systemprompt_traits::{BoxedSource, McpRegistryProvider};

pub(crate) fn mcp_registry(ctx: &AppContext) -> Result<Arc<dyn McpRegistryProvider>, BoxedSource> {
    ctx.kernel()
        .state()
        .get::<Arc<dyn McpRegistryProvider>>()
        .cloned()
        .map_err(|absent| -> BoxedSource { Box::new(absent) })
}
