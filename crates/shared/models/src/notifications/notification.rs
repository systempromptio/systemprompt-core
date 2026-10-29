//! The per-recipient notification as the API serves it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{NotificationId, NotificationStreamId, UserId};

use super::action::ActionPrompt;
use super::attention::{Attention, AttentionReason, UnknownNotificationValue};
use super::cursor::KeysetCursor;
use super::kind::NotificationKind;
use super::subject::SubjectRef;

/// Where a notification stands on its way to its delivery channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryState {
    Pending,
    Delivered,
    Held,
}

impl DeliveryState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Delivered => "delivered",
            Self::Held => "held",
        }
    }
}

impl fmt::Display for DeliveryState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DeliveryState {
    type Err = UnknownNotificationValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "delivered" => Ok(Self::Delivered),
            "held" => Ok(Self::Held),
            other => Err(UnknownNotificationValue::new("delivery_state", other)),
        }
    }
}

/// One recipient's notification row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Notification {
    pub id: NotificationId,
    pub recipient: UserId,
    pub kind: NotificationKind,
    pub stream: Option<NotificationStreamId>,
    pub actor: UserId,
    pub actor_display: Option<String>,
    pub actor_is_agent: bool,
    pub via_agent: Option<String>,
    pub subjects: Vec<SubjectRef>,
    pub attention: Attention,
    pub attention_reason: AttentionReason,
    pub action: ActionPrompt,
    pub excerpt: Option<String>,
    pub coalesce_count: i32,
    pub first_event_at: DateTime<Utc>,
    pub last_event_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub escalated_from: Option<Attention>,
    pub escalation_note: Option<String>,
    pub held_until: Option<DateTime<Utc>>,
    pub delivery_state: DeliveryState,
}

/// One page of a reader's feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NotificationPage {
    pub notifications: Vec<Notification>,
    pub next_cursor: Option<KeysetCursor>,
    pub unread: i64,
    pub unread_capped: bool,
}

/// Request body marking notifications read: the listed ids, or all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MarkRead {
    #[serde(default)]
    pub ids: Vec<NotificationId>,
    #[serde(default)]
    pub all: bool,
}

/// Request body recording the listed notifications as seen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MarkSeen {
    pub ids: Vec<NotificationId>,
}

/// Request body raising a notification to human attention.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Escalate {
    #[serde(default)]
    pub note: Option<String>,
}
