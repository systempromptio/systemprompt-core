//! Slack integration for systemprompt.io.
//!
//! This crate turns Slack into a first-class inbound surface, peer to the
//! gateway and MCP. A Slack message, slash command, or Block Kit interaction is
//! verified, mapped to a governed systemprompt identity, authorized against
//! RBAC, dispatched to an A2A agent, and answered back in Slack — under the
//! same audit pipeline every other surface gets.
//!
//! # Layered components
//!
//! - [`extension::SlackExtension`] — `Extension` registration entry-point
//!   (schemas, migrations, and the `slack` config prefix).
//! - [`SlackAppConfig`] — the per-app YAML model (`services/slack/*.yaml`),
//!   re-exported from `systemprompt_models::services`.
//! - [`signature::verify_slack_signature`] — request-signature verification.
//! - [`events`] — typed inbound payloads and their normalization.
//! - [`client::SlackClient`] — outbound Web API client (SSRF-guarded).
//! - [`blockkit`] — agent text → Block Kit rendering.
//!
//! Inbound HTTP handlers live in `systemprompt-api`; this crate owns the
//! protocol logic they delegate to. Secrets (signing secret, bot token) are
//! referenced by name and resolved from the profile secret store — never
//! inlined in config.
//!
//! # Errors
//!
//! All fallible APIs return [`error::SlackResult`] over [`error::SlackError`],
//! a `thiserror` enum composed via the shared `domain_error!` macro.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod blockkit;
pub mod client;
pub mod error;
pub mod events;
pub mod extension;
pub mod signature;

pub use error::{SlackError, SlackResult};
pub use extension::SlackExtension;
pub use systemprompt_models::services::{SlackAppConfig, SlackAuthzConfig};
