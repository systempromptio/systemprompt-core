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

pub mod resolver;
pub mod trait_impl;
pub mod validator;

pub use resolver::RegistryService;
