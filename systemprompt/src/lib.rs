#![expect(
    clippy::doc_markdown,
    reason = "README and module docs contain proper names (systemprompt, Axum, SQLx, OAuth) that the lint flags as missing backticks; rewriting each occurrence would harm prose readability"
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
//! systemprompt = { version = "0.10", features = ["full"] }
//! ```

/// Core trait surface from `systemprompt-traits`.
///
/// Includes `LlmProvider`, `ToolProvider`, `Job`, and the typed error
/// contracts (`ApiError`, `ExtensionError`, `McpErrorData`).
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod traits {
    pub use systemprompt_traits::*;
}

/// I/O-free data models from `systemprompt-models` — config structs, profile
/// types, domain DTOs.
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod models {
    pub use systemprompt_models::*;
}

/// Typed identifiers from `systemprompt-identifiers` (`UserId`, `AgentId`,
/// `TaskId`, `TraceId`, …).
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

/// In-process event bus and SSE broadcasting from `systemprompt-events`.
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

/// `RuntimeBuilder` re-export — fluent builder for embedding the CLI with
/// extensions injected at compile time.
#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::RuntimeBuilder;

/// `WebAssets` strategy re-export — controls how the runtime serves the static
/// web bundle (in-binary, on-disk, or disabled).
#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::WebAssets;

/// Typed error returned by [`RuntimeBuilder::run`].
#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::RuntimeError;

/// Agent-to-Agent (A2A) protocol surface from `systemprompt-agent` — message
/// types, task lifecycle, streaming server, agent registry.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod agent {
    pub use systemprompt_agent::*;
}

/// LLM integration surface from `systemprompt-ai` — provider selection,
/// request/response types, cost accounting.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod ai {
    pub use systemprompt_ai::*;
}

/// Model Context Protocol implementation from `systemprompt-mcp` — server
/// orchestrator, network/proxy layer, RBAC middleware.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod mcp {
    pub use systemprompt_mcp::*;
}

/// OAuth2 / OIDC / WebAuthn flows from `systemprompt-oauth`.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod oauth {
    pub use systemprompt_oauth::*;
}

/// User management domain from `systemprompt-users` — accounts, roles, scopes.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod users {
    pub use systemprompt_users::*;
}

/// Content management domain from `systemprompt-content` — pages, articles,
/// markdown ingestion.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod content {
    pub use systemprompt_content::*;
}

/// Analytics domain from `systemprompt-analytics` — request, conversation,
/// agent, tool, cost metrics.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod analytics {
    pub use systemprompt_analytics::*;
}

/// Marketplace filtering domain from `systemprompt-marketplace` — the
/// `MarketplaceFilter` trait that gates per-user visibility of
/// plugins, skills, agents, and managed MCP servers in the bridge
/// manifest.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod marketplace {
    pub use systemprompt_marketplace::*;
}

/// Background-job scheduler from `systemprompt-scheduler`.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod scheduler {
    pub use systemprompt_scheduler::*;
}

/// Static-site generator from `systemprompt-generator` — Tera-based renderer
/// driving the `web` CLI domain.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod generator {
    pub use systemprompt_generator::*;
}

/// File-storage domain from `systemprompt-files`.
#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod files {
    pub use systemprompt_files::*;
}

/// Cloud synchronisation primitives from `systemprompt-sync`.
#[cfg(feature = "sync")]
#[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
pub mod sync {
    pub use systemprompt_sync::*;
}

/// Cloud API client from `systemprompt-cloud` — credentials bootstrap, tenant
/// management, deployment.
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
    /// `ProfileBootstrap` loader and its typed error.
    #[cfg(feature = "config")]
    pub use systemprompt_config::{ProfileBootstrap, ProfileBootstrapError};

    /// Profile schema types from `systemprompt-models`.
    pub use systemprompt_models::profile::{
        CloudConfig, CloudValidationMode, Profile, ProfileStyle,
    };
}

/// Cloud credentials bootstrap from `systemprompt-cloud` — loads OAuth client
/// credentials and tenant identity at startup.
#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod credentials {
    pub use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError};
}

/// Curated re-exports for ergonomic `use systemprompt::prelude::*`. See
/// [`prelude`] for the full list.
pub mod prelude;

pub use crate::prelude::*;
