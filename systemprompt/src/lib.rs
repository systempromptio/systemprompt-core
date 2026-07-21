#![expect(
    clippy::doc_markdown,
    reason = "README contains brand names and acronyms that doc_markdown would over-flag"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
//! # Feature flags
//!
//! | Feature | Pulls in | Use case |
//! |---------|----------|----------|
//! | `core` *(default)* | `traits`, `models`, `identifiers`, `extension`, `template-provider` | Author extensions, share types, no I/O. |
//! | `database` | `systemprompt-database`, `sqlx` | SQLx-backed `DbPool` and repository helpers. |
//! | `config` | `systemprompt-config` | Profile, secrets, and credential bootstrap loaders. |
//! | `mcp` | `rmcp` | Implement Model Context Protocol servers. |
//! | `api` | `systemprompt-api`, `systemprompt-runtime`, `axum` (implies `core` + `database`) | HTTP server, `AppContext`, Axum router. |
//! | `sync` | `systemprompt-sync` | Cloud synchronisation primitives. |
//! | `cloud` | `systemprompt-cloud` | Cloud API client, credentials bootstrap, OAuth. |
//! | `logging` | `systemprompt-logging` | Tracing setup with the workspace's layer stack. |
//! | `loader` | `systemprompt-loader` | Filesystem and module discovery. |
//! | `events` | `systemprompt-events` | In-process event bus and SSE plumbing. |
//! | `client` | `systemprompt-client` | HTTP API client used by the CLI. |
//! | `security` | `systemprompt-security` | JWT, scope/RBAC, secret scanning, rate limit. |
//! | `cli` | `systemprompt-cli` | The `systemprompt` CLI as a library entry point. |
//! | `runtime` | `cli` + extension injection | `RuntimeBuilder` for embedding with custom extensions. |
//! | `test-utils` | `cloud` | Enables `cloud` for test scaffolding; not for production. |
//! | `full` | All of the above plus all domain crates (`agent`, `ai`, `mcp`, `oauth`, `users`, `content`, `analytics`, `scheduler`, `generator`, `files`) | Building a product binary. |
//!
//! ```toml
//! systemprompt = { version = "0.21.1", features = ["full"] }
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Includes `LlmProvider`, `ToolProvider`, `Job`, and the typed error
/// contracts (`ApiError`, `ExtensionError`, `McpErrorData`).
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod traits {
    pub use systemprompt_traits::*;
}

/// I/O-free: config structs, profile types, domain DTOs.
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod models {
    pub use systemprompt_models::*;
}

/// `UserId`, `AgentId`, `TaskId`, `TraceId`, and the rest of the wrappers.
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod identifiers {
    pub use systemprompt_identifiers::*;
}

/// Compile-time extension framework: the `Extension` trait, typed variants,
/// `register_extension!` macro, and registry.
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod extension {
    pub use systemprompt_extension::*;
}

/// Template provider trait surface for custom rendering backends (Tera,
/// Handlebars, MJML, …).
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod template_provider {
    pub use systemprompt_template_provider::*;
}

/// SQLx-backed database abstraction: `DbPool`, `DatabaseProvider`,
/// repositories, introspection.
#[cfg(feature = "database")]
#[cfg_attr(docsrs, doc(cfg(feature = "database")))]
pub mod database {
    pub use systemprompt_database::*;
}

/// Tracing/logging setup helpers (startup-mode gating, layered subscribers).
#[cfg(feature = "logging")]
#[cfg_attr(docsrs, doc(cfg(feature = "logging")))]
pub mod logging {
    pub use systemprompt_logging::*;
}

/// Profile / secrets / credentials configuration loaders. Drives the
/// `ProfileBootstrap → SecretsBootstrap → CredentialsBootstrap → Config`
/// sequence.
#[cfg(feature = "config")]
#[cfg_attr(docsrs, doc(cfg(feature = "config")))]
pub mod config {
    pub use systemprompt_config::*;
}

/// Filesystem and module discovery for services, plugins, and config files.
#[cfg(feature = "loader")]
#[cfg_attr(docsrs, doc(cfg(feature = "loader")))]
pub mod loader {
    pub use systemprompt_loader::*;
}

/// In-process event bus and SSE broadcasting.
#[cfg(feature = "events")]
#[cfg_attr(docsrs, doc(cfg(feature = "events")))]
pub mod events {
    pub use systemprompt_events::*;
}

/// HTTP API client used by the CLI and external tooling to drive a running
/// instance.
#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client {
    pub use systemprompt_client::*;
}

/// Security primitives: JWT verification, scope/RBAC, secret scanning,
/// rate-limit middleware.
#[cfg(feature = "security")]
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub mod security {
    pub use systemprompt_security::*;
}

/// Application runtime / `AppContext` wiring. Construct via `AppContextBuilder`
/// from the prelude.
#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod system {
    pub use systemprompt_runtime::*;
}

/// HTTP server entry: Axum router, middleware stack, listener bootstrap.
#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod api {
    pub use systemprompt_api::*;
}

/// CLI entry surface: `run`, `CliConfig`, `OutputFormat`, `ColorMode`,
/// `VerbosityLevel`.
#[cfg(feature = "cli")]
#[cfg_attr(docsrs, doc(cfg(feature = "cli")))]
pub mod cli {
    pub use systemprompt_cli::{CliConfig, ColorMode, OutputFormat, VerbosityLevel, run};
}

/// `RuntimeBuilder` for embedding the platform with compile-time injected
/// extensions and a custom web-asset strategy.
#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub mod runtime;

#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::RuntimeBuilder;

/// Controls how the runtime serves the static web bundle: in-binary, on-disk,
/// or disabled.
#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::WebAssets;

#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::RuntimeError;

/// Agent-to-Agent (A2A) protocol: message types, task lifecycle, streaming
/// server, agent registry.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod agent {
    pub use systemprompt_agent::*;
}

/// Provider selection, request/response types, cost accounting.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod ai {
    pub use systemprompt_ai::*;
}

/// Server orchestrator, network/proxy layer, RBAC middleware.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod mcp {
    pub use systemprompt_mcp::*;
}

/// OAuth2, OIDC, and WebAuthn flows.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod oauth {
    pub use systemprompt_oauth::*;
}

/// Accounts, roles, scopes.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod users {
    pub use systemprompt_users::*;
}

/// Pages, articles, markdown ingestion.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod content {
    pub use systemprompt_content::*;
}

/// Request, conversation, agent, tool, and cost metrics.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod analytics {
    pub use systemprompt_analytics::*;
}

/// The `MarketplaceFilter` trait, which gates per-user visibility of plugins,
/// skills, agents, and managed MCP servers in the bridge manifest.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod marketplace {
    pub use systemprompt_marketplace::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod scheduler {
    pub use systemprompt_scheduler::*;
}

/// Inbound Events API, slash commands, and Block Kit interactivity dispatched
/// to governed agents.
#[cfg(feature = "slack")]
#[cfg_attr(docsrs, doc(cfg(feature = "slack")))]
pub mod slack {
    pub use systemprompt_slack::*;
}

/// Inbound Bot Framework activities, token validation, Adaptive Card rendering.
#[cfg(feature = "teams")]
#[cfg_attr(docsrs, doc(cfg(feature = "teams")))]
pub mod teams {
    pub use systemprompt_teams::*;
}

/// Tera-based renderer driving the `web` CLI domain.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod generator {
    pub use systemprompt_generator::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod files {
    pub use systemprompt_files::*;
}

#[cfg(feature = "sync")]
#[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
pub mod sync {
    pub use systemprompt_sync::*;
}

/// Credentials bootstrap, tenant management, deployment.
#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod cloud {
    pub use systemprompt_cloud::*;
}

/// Profile types — the on-disk profile schema (`Profile`, `CloudConfig`,
/// `ProfileStyle`, `CloudValidationMode`) plus the `ProfileBootstrap` loader
/// when the `config` feature is enabled.
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod profile {
    #[cfg(feature = "config")]
    pub use systemprompt_config::{ProfileBootstrap, ProfileBootstrapError};

    pub use systemprompt_models::profile::{
        CloudConfig, CloudValidationMode, Profile, ProfileStyle,
    };
}

/// Loads OAuth client credentials and tenant identity at startup.
#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod credentials {
    pub use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError};
}

/// Curated re-exports for `use systemprompt::prelude::*`.
pub mod prelude;

pub use crate::prelude::*;
