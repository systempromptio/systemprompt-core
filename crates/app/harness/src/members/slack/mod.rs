//! Slack, as far as a desktop can honestly see it through the instance.
//!
//! The instance's Slack routes are inbound webhooks: Slack calls the
//! instance, which answers in Slack itself. It serves no client API for Slack
//! and exposes no per-user Slack link or enablement state. So
//! [`SlackExtension`] (id `slack`, which depends on `notifications`) holds
//! no Slack credential and never calls Slack: its [`SlackHandle`] filters the
//! reader's notification inbox down to the Slack-originated items
//! ([`is_slack_originated`]) and derives a [`SlackStatus`] from them alone.
//! Everything is withheld unless the applied instance enables both `slack`
//! and `notifications`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod extension;
mod handle;
mod origin;
mod status;

pub use extension::SlackExtension;
pub use handle::{SlackChanges, SlackHandle};
pub use origin::is_slack_originated;
pub use status::SlackStatus;
