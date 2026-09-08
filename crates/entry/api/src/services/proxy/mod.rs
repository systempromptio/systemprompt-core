//! Reverse proxy from the API gateway to local MCP and agent backends.
//!
//! [`ProxyEngine`] is the entry point; the `auth` submodule is the single
//! authorization boundary for proxied `/api/v1/mcp/*` and `/api/v1/agents/*`
//! traffic, emitting RFC 9728 OAuth challenges on unauthenticated requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod audit;
pub mod auth;
mod backend;
mod client;
pub mod engine;
mod errors;
pub mod resolver;

pub use engine::{ProxyEngine, ProxyKind, ProxyTarget};
pub use errors::ProxyError;
