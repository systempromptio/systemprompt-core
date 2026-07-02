//! Reverse proxy from the API gateway to local MCP and agent backends.
//!
//! [`ProxyEngine`] is the entry point; the `auth` submodule is the single
//! authorization boundary for proxied `/api/v1/mcp/*` and `/api/v1/agents/*`
//! traffic, emitting RFC 9728 OAuth challenges on unauthenticated requests.

mod audit;
pub mod auth;
mod backend;
mod client;
mod engine;
mod errors;
mod resolver;

pub use engine::{ProxyEngine, ProxyKind, ProxyTarget};

#[cfg(feature = "test-api")]
pub use audit::test_api;
#[cfg(feature = "test-api")]
pub use auth::test_api as auth_test_api;
#[cfg(feature = "test-api")]
pub use engine::test_api as engine_test_api;
