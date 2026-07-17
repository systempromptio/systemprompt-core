//! MCP server registry.
//!
//! Resolves configured servers from the loader config and adapts them onto
//! the `McpRegistry`, `McpToolProvider`, and `McpRegistryProvider` traits.
//!
//! The registry is owner-scoped: every `McpServerConfig` it materialises is
//! attributed to the [`UserId`][uid] passed to [`RegistryService::new`]. The
//! platform constructs one instance during `AppContext` bootstrap with the
//! resolved system-admin id; callers obtain it via `AppContext::mcp_registry`.
//!
//! [uid]: systemprompt_identifiers::UserId
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod resolver;
pub mod trait_impl;
pub mod validator;

pub use resolver::RegistryService;
