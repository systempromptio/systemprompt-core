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
//! | `cloud` | `systemprompt-cloud` | Cloud API client, credentials bootstrap, OAuth. |
//! | `logging` | `systemprompt-logging` | Tracing setup with the workspace's layer stack. |
//! | `loader` | `systemprompt-loader` | Filesystem and module discovery. |
//! | `events` | `systemprompt-events` | In-process event bus and SSE plumbing. |
//! | `storage` | `systemprompt-storage` | File storage backends and the shared-mount probe. |
//! | `client` | `systemprompt-client` | HTTP API client used by the CLI. |
//! | `security` | `systemprompt-security` | JWT, scope/RBAC, secret scanning, rate limit. |
//! | `cli` | `systemprompt-cli` | The `systemprompt` CLI as a library entry point. |
//! | `runtime` | `cli` + extension injection | `RuntimeBuilder` for embedding with custom extensions. |
//! | `analytics` | `systemprompt-analytics` | Request, conversation, agent, tool, and cost metrics without the rest of `full`. |
//! | `slack` | `systemprompt-slack` | Slack Events API, slash commands, interactivity. Opt-in: not part of `full`. |
//! | `teams` | `systemprompt-teams` | Microsoft Teams Bot Framework activities. Opt-in: not part of `full`. |
//! | `full` | `api`, `mcp`, `cloud`, `cli`, `config`, `logging`, `loader`, `events`, `storage`, `client`, `security`, `analytics`, and the domain crates (`agent`, `ai`, `mcp`, `oauth`, `users`, `content`, `marketplace`, `scheduler`, `generator`, `files`) | Building a product binary. `slack` and `teams` stay opt-in. |
//!
//! ```toml
//! systemprompt = { version = "0.55.0", features = ["full"] }
//! ```
//!
//! Every crate is reachable as a module of the same name
//! (`systemprompt::models`, `systemprompt::agent`, …) gated on its feature. The
//! curated [`prelude`] is opt-in — `use systemprompt::prelude::*` — and is not
//! re-exported at the crate root, so the root namespace stays the module map.
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
/// `ProfileBootstrap → SecretsBootstrap → CredentialsBootstrap → Config`
/// sequence.
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

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod api {
    pub use systemprompt_api::*;
}

#[cfg(feature = "cli")]
#[cfg_attr(docsrs, doc(cfg(feature = "cli")))]
pub mod cli {
    pub use systemprompt_cli::{CliConfig, ColorMode, OutputFormat, VerbosityLevel, run};
}

#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub mod runtime;

#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::RuntimeBuilder;

#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::WebAssets;

#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub use runtime::RuntimeError;

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod agent {
    pub use systemprompt_agent::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod ai {
    pub use systemprompt_ai::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod mcp {
    pub use systemprompt_mcp::{register_artifact_theme, register_ui_renderer, *};
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod oauth {
    pub use systemprompt_oauth::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod users {
    pub use systemprompt_users::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod content {
    pub use systemprompt_content::*;
}

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
pub mod scheduler {
    pub use systemprompt_scheduler::*;
}

#[cfg(feature = "slack")]
#[cfg_attr(docsrs, doc(cfg(feature = "slack")))]
pub mod slack {
    pub use systemprompt_slack::*;
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

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod files {
    pub use systemprompt_files::*;
}

#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod cloud {
    pub use systemprompt_cloud::*;
}

/// Profile types — the on-disk profile schema.
///
/// `Profile`, `CloudConfig`, `ProfileStyle` and `CloudValidationMode`, plus the
/// `ProfileBootstrap` loader when the `config` feature is enabled and the
/// `ServicesBootstrap` cell (provider catalog, gateway routes) when `loader`
/// is.
#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod profile {
    #[cfg(feature = "config")]
    pub use systemprompt_config::{ProfileBootstrap, ProfileBootstrapError};
    #[cfg(feature = "loader")]
    pub use systemprompt_loader::ServicesBootstrap;

    pub use systemprompt_models::profile::{
        CloudConfig, CloudValidationMode, Profile, ProfileStyle,
    };
}

#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod credentials {
    pub use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError};
}

pub mod prelude;
