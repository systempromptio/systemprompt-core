//! Repository inputs and outputs, and the stored-row decoding behind them.
//!
//! Stored enum columns (`attention`, `attention_reason`, audience subjects) are
//! parsed with their `FromStr`; a value the vocabulary does not know is a
//! [`RepositoryError::Decode`], never a silent default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use systemprompt_identifiers::{
    EventOutboxId, NotificationId, NotificationStreamId, NotificationSubscriptionId, UserId,
};
use systemprompt_models::notifications::{
    ActionPrompt, Attention, AttentionReason, DeliveryState, KeysetCursor, Notification,
    NotificationKind, SubjectRef, SubscriptionDecl,
};
use systemprompt_traits::RepositoryError;

/// A subscription row: its declaration plus id and origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionView {
    pub id: NotificationSubscriptionId,
    pub decl: SubscriptionDecl,
    pub source: String,
}

/// One fact's rows as the pipeline persists them: the columns every
/// recipient shares, and each recipient's own.
#[derive(Debug, Clone)]
pub struct NewNotifications {
    pub kind: NotificationKind,
    pub stream: Option<NotificationStreamId>,
    pub actor: UserId,
    pub actor_is_agent: bool,
    pub via_agent: Option<String>,
    pub subjects: Vec<SubjectRef>,
    pub action: ActionPrompt,
    pub event_at: DateTime<Utc>,
    pub outbox_id: EventOutboxId,
    pub recipients: Vec<NewRecipient>,
}

/// One recipient's columns of a [`NewNotifications`] batch.
#[derive(Debug, Clone)]
pub struct NewRecipient {
    pub user: UserId,
    pub attention: Attention,
    pub reason: AttentionReason,
    pub excerpt: Option<String>,
    pub coalesce_key: String,
    pub held_until: Option<DateTime<Utc>>,
}

/// A persisted row: whether it was inserted or folded into an unread one,
/// and where it stands on its way to the channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedRow {
    pub id: NotificationId,
    pub recipient: UserId,
    pub inserted: bool,
    pub delivery_state: DeliveryState,
}

/// A keyset feed page request, newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedFilter {
    pub since: Option<KeysetCursor>,
    pub limit: i64,
    pub attention: Option<Attention>,
    pub unread_only: bool,
}

pub(crate) fn parse<T>(field: &'static str, value: &str) -> Result<T, RepositoryError>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    T::from_str(value).map_err(|source| RepositoryError::decode(field, source))
}

pub(crate) fn parse_opt<T>(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<T>, RepositoryError>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value.map(|v| parse(field, v)).transpose()
}

pub(crate) fn to_u32(
    field: &'static str,
    value: Option<i32>,
) -> Result<Option<u32>, RepositoryError> {
    value
        .map(|v| u32::try_from(v).map_err(|source| RepositoryError::decode(field, source)))
        .transpose()
}

pub(crate) fn to_i32(
    field: &'static str,
    value: Option<u32>,
) -> Result<Option<i32>, RepositoryError> {
    value
        .map(|v| {
            i32::try_from(v).map_err(|_overflow| {
                RepositoryError::invalid_argument(field, "exceeds the INT range")
            })
        })
        .transpose()
}

pub(crate) struct NotificationRow {
    pub(crate) id: NotificationId,
    pub(crate) recipient_user_id: UserId,
    pub(crate) kind: String,
    pub(crate) stream_id: Option<NotificationStreamId>,
    pub(crate) actor_user_id: UserId,
    pub(crate) actor_display: Option<String>,
    pub(crate) actor_is_agent: bool,
    pub(crate) via_agent: Option<String>,
    pub(crate) subjects: Json<Vec<SubjectRef>>,
    pub(crate) attention: String,
    pub(crate) attention_reason: String,
    pub(crate) action: Json<ActionPrompt>,
    pub(crate) excerpt: Option<String>,
    pub(crate) coalesce_count: i32,
    pub(crate) first_event_at: DateTime<Utc>,
    pub(crate) last_event_at: DateTime<Utc>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) read_at: Option<DateTime<Utc>>,
    pub(crate) escalated_from: Option<String>,
    pub(crate) escalation_note: Option<String>,
    pub(crate) held_until: Option<DateTime<Utc>>,
    pub(crate) delivery_state: String,
    pub(crate) delivery_seq: i64,
}

impl TryFrom<NotificationRow> for Notification {
    type Error = RepositoryError;

    fn try_from(row: NotificationRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            recipient: row.recipient_user_id,
            kind: parse("notifications.kind", &row.kind)?,
            stream: row.stream_id,
            actor: row.actor_user_id,
            actor_display: row.actor_display,
            actor_is_agent: row.actor_is_agent,
            via_agent: row.via_agent,
            subjects: row.subjects.0,
            attention: parse("notifications.attention", &row.attention)?,
            attention_reason: parse("notifications.attention_reason", &row.attention_reason)?,
            action: row.action.0,
            excerpt: row.excerpt,
            coalesce_count: row.coalesce_count,
            first_event_at: row.first_event_at,
            last_event_at: row.last_event_at,
            created_at: row.created_at,
            read_at: row.read_at,
            escalated_from: parse_opt(
                "notifications.escalated_from",
                row.escalated_from.as_deref(),
            )?,
            escalation_note: row.escalation_note,
            held_until: row.held_until,
            delivery_state: parse("notifications.delivery_state", &row.delivery_state)?,
        })
    }
}
