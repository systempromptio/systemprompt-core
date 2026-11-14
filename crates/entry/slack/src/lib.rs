//! `systemprompt-slack-entry` — the Slack entry surface.
//!
//! [`SlackExtension`] is the capability's one registration: the `slack`
//! config prefix and schema, and, behind the `http` feature, the inbound
//! routes under `/api/v1/slack` (Events API, slash commands, Block Kit
//! interactions). The routes are declared as an `ExtensionRouter` in the
//! `Messaging` route group with the `SlackSignature` handler-verified policy:
//! each handler verifies the request signature over the raw body before it
//! acts. A verified message is dispatched through the shared
//! `MessagingDispatcher` the agent capability binds into the extension state;
//! this crate never names the agent or the API server's internals.
//!
//! When the profile disables the `slack` extension, the routes are not
//! mounted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod extension;
#[cfg(feature = "http")]
pub mod http;

pub use extension::SlackExtension;
