//! `systemprompt-notifications-entry` — the notifications entry surface.
//!
//! The `http` feature carries the capability's routes: the user feed, read,
//! seen, escalation, streams, subscriptions and personal rules under
//! `/api/v1/notifications`, the admin routing simulator under
//! `/api/v1/admin/notifications`, and the bridge inbox under
//! `/v1/bridge/notifications*`. [`extension::NotificationsHttpExtension`]
//! contributes the first two as route sets and the inbox as its gateway
//! route, so the API server mounts all three without naming this crate. The
//! `cli` feature is reserved for the capability's commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod extension;
#[cfg(feature = "http")]
pub mod http;
