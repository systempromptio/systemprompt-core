//! Realtime event names and payloads pushed to connected readers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::NotificationId;

use super::attention::Attention;
use super::notification::Notification;

pub const NOTIFICATION_CREATED: &str = "notification.created";
pub const NOTIFICATION_READ: &str = "notification.read";
pub const NOTIFICATION_ESCALATED: &str = "notification.escalated";

/// Payload of [`NOTIFICATION_CREATED`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NotificationCreatedPayload {
    pub notification: Notification,
    pub unread: i64,
}

/// Payload of [`NOTIFICATION_READ`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NotificationReadPayload {
    pub ids: Vec<NotificationId>,
    pub unread: i64,
}

/// Payload of [`NOTIFICATION_ESCALATED`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NotificationEscalatedPayload {
    pub id: NotificationId,
    pub attention: Attention,
}
