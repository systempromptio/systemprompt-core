//! Where notification declarations live under the services root.
//!
//! `notifications/streams.yaml`, `notifications/rules.yaml` and
//! `notifications/subscriptions.yaml` are each parsed into
//! `systemprompt_models::notifications::NotificationsDoc` and merged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const NOTIFICATIONS_DIR: &str = "notifications";
pub const STREAMS_FILE: &str = "streams.yaml";
pub const RULES_FILE: &str = "rules.yaml";
pub const SUBSCRIPTIONS_FILE: &str = "subscriptions.yaml";
