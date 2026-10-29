//! The event a producer appends to the outbox for the notifications consumer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{NotificationStreamId, UserId};

use super::attention::Attention;
use super::kind::NotificationKind;
use super::subject::SubjectRef;

pub const NOTIFICATIONS_CONSUMER: &str = "notifications";
pub const NOTIFICATION_EVENT_VERSION: u32 = 1;

/// A user the producer addresses directly, optionally at a chosen attention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DirectRecipient {
    pub user: UserId,
    pub attention: Option<Attention>,
}

/// One thing that happened, before routing decides who hears about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NotificationEvent {
    pub kind: NotificationKind,
    pub actor: UserId,
    pub actor_is_agent: bool,
    pub via_agent: Option<String>,
    pub subjects: Vec<SubjectRef>,
    pub stream: Option<NotificationStreamId>,
    pub direct_recipients: Vec<DirectRecipient>,
    pub sender_attention: Option<Attention>,
    pub excerpt: Option<String>,
    // JSON: the producer's payload is the event's free-form contract field.
    pub payload: serde_json::Value,
    pub occurred_at: DateTime<Utc>,
}

impl NotificationEvent {
    #[must_use]
    pub const fn new(kind: NotificationKind, actor: UserId, occurred_at: DateTime<Utc>) -> Self {
        Self {
            kind,
            actor,
            actor_is_agent: false,
            via_agent: None,
            subjects: Vec::new(),
            stream: None,
            direct_recipients: Vec::new(),
            sender_attention: None,
            excerpt: None,
            payload: serde_json::Value::Null,
            occurred_at,
        }
    }

    #[must_use]
    pub const fn with_actor_is_agent(mut self, actor_is_agent: bool) -> Self {
        self.actor_is_agent = actor_is_agent;
        self
    }

    #[must_use]
    pub fn with_via_agent(mut self, via_agent: impl Into<String>) -> Self {
        self.via_agent = Some(via_agent.into());
        self
    }

    #[must_use]
    pub fn with_subject(mut self, subject: SubjectRef) -> Self {
        self.subjects.push(subject);
        self
    }

    #[must_use]
    pub fn with_subjects(mut self, subjects: impl IntoIterator<Item = SubjectRef>) -> Self {
        self.subjects.extend(subjects);
        self
    }

    #[must_use]
    pub fn with_stream(mut self, stream: NotificationStreamId) -> Self {
        self.stream = Some(stream);
        self
    }

    #[must_use]
    pub fn with_direct_recipient(mut self, user: UserId, attention: Option<Attention>) -> Self {
        self.direct_recipients
            .push(DirectRecipient { user, attention });
        self
    }

    #[must_use]
    pub fn with_direct_recipients(
        mut self,
        recipients: impl IntoIterator<Item = DirectRecipient>,
    ) -> Self {
        self.direct_recipients.extend(recipients);
        self
    }

    #[must_use]
    pub const fn with_sender_attention(mut self, attention: Attention) -> Self {
        self.sender_attention = Some(attention);
        self
    }

    #[must_use]
    pub fn with_excerpt(mut self, excerpt: impl Into<String>) -> Self {
        self.excerpt = Some(excerpt.into());
        self
    }

    #[must_use]
    // JSON: the producer's payload is the event's free-form contract field.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }
}
