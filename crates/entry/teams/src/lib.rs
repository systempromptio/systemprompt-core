//! `systemprompt-teams-entry` — the Microsoft Teams entry surface.
//!
//! [`TeamsExtension`] is the capability's one member: the `teams` config
//! section, and, behind the `http` feature, the inbound Bot Framework route
//! under `/api/v1/teams`. The route is one `RouteSet` in the `Messaging`
//! route group with the `TeamsBotToken`
//! handler-verified policy: the handler validates the activity's Bot
//! Connector bearer before it acts. A verified message is dispatched through
//! the shared `MessagingDispatcher` the agent capability binds into the
//! extension state; this crate never names the agent or the API server's
//! internals.
//!
//! When the profile disables the `teams` member, the route is not mounted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod extension;
#[cfg(feature = "http")]
pub mod http;

pub use extension::TeamsExtension;
