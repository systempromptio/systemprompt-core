//! Microsoft Teams integration for systemprompt.io.
//!
//! This crate turns Teams into a first-class inbound surface, peer to the
//! gateway, MCP, and Slack. A Teams message or invoke activity is verified,
//! mapped to a governed systemprompt identity, authorized against RBAC,
//! dispatched to an A2A agent, and answered back in Teams — under the same
//! audit pipeline every other surface gets.
//!
//! Unlike Slack's static HMAC signing secret and bot token, Teams uses the
//! Azure Bot Service identity model: inbound activities are authenticated by an
//! `RS256` JWT validated against the Bot Connector JWKS, and outbound replies
//! use an `OAuth2` client-credentials token. There is no official Bot Framework
//! SDK for Rust, so the wire is implemented here over `jsonwebtoken` and
//! `reqwest`.
//!
//! # Layered components
//!
//! - [`extension::TeamsExtension`] — `Extension` registration entry-point
//!   (schemas, migrations, and the `teams` config prefix).
//! - [`TeamsAppConfig`] — the per-app YAML model (`services/teams/*.yaml`),
//!   re-exported from `systemprompt_models::services`.
//! - [`auth::ActivityTokenVerifier`] — inbound activity-token validation.
//! - [`token::TokenProvider`] — outbound client-credentials token acquisition.
//! - [`activities`] — typed inbound activities and their normalization.
//! - [`client::TeamsClient`] — outbound Bot Connector client (SSRF-guarded).
//! - [`cards`] — agent text → Adaptive Card rendering.
//!
//! Inbound HTTP handlers live in `systemprompt-api`; this crate owns the
//! protocol logic they delegate to. Secrets (the app password) are referenced
//! by name and resolved from the profile secret store — never inlined in
//! config.
//!
//! # Errors
//!
//! All fallible APIs return [`error::TeamsResult`] over [`error::TeamsError`],
//! a `thiserror` enum composed via the shared `domain_error!` macro.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod activities;
pub mod auth;
pub mod cards;
pub mod client;
pub mod error;
pub mod extension;
pub mod token;

pub use error::{TeamsError, TeamsResult};
pub use extension::TeamsExtension;
pub use systemprompt_models::services::{TeamsAppConfig, TeamsAuthzConfig};
