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
//! | `core` *(default)* | `traits`, `models`, `wire`, `manifest`, `identifiers`, `extension`, `template-provider` | Author extensions, share types, no I/O. |
//! | `database` | `systemprompt-database`, `sqlx` | SQLx-backed `DbPool` and repository helpers. |
//! | `config` | `systemprompt-config` | Profile, secrets, and credential bootstrap loaders. |
//! | `mcp` | `rmcp` | Implement Model Context Protocol servers with `rmcp`. The `systemprompt::mcp` module (`systemprompt-mcp`) is gated on `full`, not on this flag. |
//! | `api` | `systemprompt-api`, `systemprompt-gateway`, `systemprompt-runtime`, `systemprompt-oauth-issuance`, `axum` (implies `core` + `database`) | HTTP server, AI gateway, `AppContext`, Axum router, OAuth token issuance. `systemprompt-api` always compiles `systemprompt-slack`, `systemprompt-teams` and the runtime's `geolocation` (MaxMind) feature. |
//! | `cloud` | `systemprompt-cloud` | Cloud API client, credentials bootstrap, OAuth. |
//! | `logging` | `systemprompt-logging` | Tracing setup with the workspace's layer stack. |
//! | `loader` | `systemprompt-loader` | Filesystem and module discovery. |
//! | `events` | `systemprompt-events` | In-process event bus and SSE plumbing. |
//! | `storage` | `systemprompt-storage` | File storage backends and the shared-mount probe. |
//! | `client` | `systemprompt-client` | HTTP API client used by the CLI. |
//! | `security` | `systemprompt-security`, `systemprompt-governance` | JWT, scope/RBAC, secret scanning, rate limit. |
//! | `cli` | `systemprompt-cli` (which depends on `systemprompt-api`, so it compiles the `api` crate graph) | The `systemprompt` CLI as a library entry point. |
//! | `runtime` | `cli` + `systemprompt-extension` | `RuntimeBuilder` for embedding the CLI in a custom binary. The `systemprompt::runtime` module also compiles under `full`, where it re-exports the document store runtime as `systemprompt::runtime::documents` (the document MCP tools are `systemprompt::runtime::documents::mcp`). |
//! | `analytics` | `systemprompt-analytics` | Request, conversation, agent, tool, and cost metrics without the rest of `full`. |
//! | `slack` | the `systemprompt::slack` module | Slack Events API, slash commands, interactivity. Not part of `full`; the flag adds only the module re-export, since `api` compiles the crate regardless. |
//! | `teams` | the `systemprompt::teams` module | Microsoft Teams Bot Framework activities. Not part of `full`; the flag adds only the module re-export, since `api` compiles the crate regardless. |
//! | `agent-runs` | `systemprompt-agent-runs` | Durable, leased, governed coding-agent runs (Claude Code, Codex CLI) with a common event contract. In `full`. |
//! | `notifications` | the `systemprompt::notifications` module, and the notification routes, `documents_inbox` tool and outbox consumer in the runtime and API | Notification streams, rules, the per-user feed and the outbox consumer. In `full`, not in `core`. Without it, comments and shares commit without recording a notification and a comment carrying `mention_options` is refused with `unsupported_op`. |
//! | `files-office` | the `systemprompt::files` module with `systemprompt-files/office` | The `.docx` document codec. In `full`. |
//! | `full` | `api`, `mcp`, `cloud`, `cli`, `config`, `logging`, `loader`, `events`, `storage`, `client`, `security`, `analytics`, and the domain crates (`agent`, `ai`, `mcp`, `oauth`, `users`, `content`, `marketplace`, `scheduler`, `generator`, `files`, `notifications`) | Building a product binary. The `slack` and `teams` modules stay opt-in. |
//!
//! ```toml
//! systemprompt = { version = "0.75.0", features = ["full"] }
//! ```
//!
//! Crates are reachable as a module of the same name (`systemprompt::models`,
//! `systemprompt::agent`, …) gated on its feature, with three exceptions:
//! `systemprompt-runtime` is `systemprompt::system`, `systemprompt::cli`
//! re-exports only the CLI entry points, and `systemprompt-templates` is not
//! re-exported. The curated [`prelude`] is opt-in — `use
//! systemprompt::prelude::*` — and is not re-exported at the crate root, so the
//! root namespace stays the module map.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod traits {
    pub use systemprompt_traits::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod models {
    pub use systemprompt_models::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod wire {
    pub use systemprompt_wire::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod manifest {
    pub use systemprompt_manifest::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod identifiers {
    pub use systemprompt_identifiers::*;
}

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

#[cfg(feature = "database")]
#[cfg_attr(docsrs, doc(cfg(feature = "database")))]
pub mod database {
    pub use systemprompt_database::*;
}

#[cfg(feature = "logging")]
#[cfg_attr(docsrs, doc(cfg(feature = "logging")))]
pub mod logging {
    pub use systemprompt_logging::*;
}

/// Profile / secrets / credentials configuration loaders. Drives the
/// `LoadedProfile → SecretsService → BootContext → Config` sequence.
#[cfg(feature = "config")]
#[cfg_attr(docsrs, doc(cfg(feature = "config")))]
pub mod config {
    pub use systemprompt_config::*;
}

#[cfg(feature = "loader")]
#[cfg_attr(docsrs, doc(cfg(feature = "loader")))]
pub mod loader {
    pub use systemprompt_loader::*;
}

#[cfg(feature = "events")]
#[cfg_attr(docsrs, doc(cfg(feature = "events")))]
pub mod events {
    pub use systemprompt_events::*;
}

#[cfg(feature = "storage")]
#[cfg_attr(docsrs, doc(cfg(feature = "storage")))]
pub mod storage {
    pub use systemprompt_storage::*;
}

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client {
    pub use systemprompt_client::*;
}

#[cfg(feature = "security")]
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub use systemprompt_security as security;

#[cfg(feature = "security")]
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub use systemprompt_governance as governance;

/// Application runtime / `AppContext` wiring. Construct via `AppContextBuilder`
/// from the prelude.
#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod system {
    pub use systemprompt_runtime::*;
}

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod api {
    pub use systemprompt_api::*;
}

/// The AI gateway: protocol translation, gateway policies and the
/// safety-scanner, route-selector and system-prompt-override extension
/// contracts.
#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod gateway {
    pub use systemprompt_gateway::*;
}

#[cfg(feature = "cli")]
#[cfg_attr(docsrs, doc(cfg(feature = "cli")))]
pub mod cli {
    pub use systemprompt_cli::{CliConfig, ColorMode, OutputFormat, VerbosityLevel, run};
}

#[cfg(any(feature = "runtime", feature = "full"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "runtime", feature = "full"))))]
pub mod runtime;

#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::RuntimeBuilder;

#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::RuntimeError;

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod agent;

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod ai;

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod mcp {
    pub use systemprompt_mcp::{register_artifact_theme, register_ui_renderer, *};
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod oauth;

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod users;

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod content;

#[cfg(feature = "analytics")]
#[cfg_attr(docsrs, doc(cfg(feature = "analytics")))]
pub mod analytics {
    pub use systemprompt_analytics::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod marketplace {
    pub use systemprompt_marketplace::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod scheduler;

#[cfg(feature = "slack")]
#[cfg_attr(docsrs, doc(cfg(feature = "slack")))]
pub mod slack {
    pub use systemprompt_slack::*;
}

#[cfg(feature = "agent-runs")]
#[cfg_attr(docsrs, doc(cfg(feature = "agent-runs")))]
pub mod agent_runs {
    pub use systemprompt_agent_runs::*;
}

#[cfg(feature = "teams")]
#[cfg_attr(docsrs, doc(cfg(feature = "teams")))]
pub mod teams {
    pub use systemprompt_teams::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod generator {
    pub use systemprompt_generator::*;
}

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod sso;

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod oauth_issuance {
    pub use systemprompt_oauth_issuance::*;
}

#[cfg(any(feature = "full", feature = "files-office"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "full", feature = "files-office"))))]
pub mod files {
    pub use systemprompt_files::{register_agent_access_policy, *};
}

#[cfg(any(feature = "full", feature = "files-office"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "full", feature = "files-office"))))]
pub use systemprompt_files::register_agent_access_policy;

#[cfg(feature = "notifications")]
#[cfg_attr(docsrs, doc(cfg(feature = "notifications")))]
pub mod notifications {
    pub use systemprompt_notifications::*;
}

#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod cloud {
    pub use systemprompt_cloud::*;
}

/// Profile types — the on-disk profile schema.
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod profile;

#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod credentials {
    pub use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError};
}

pub mod prelude;
