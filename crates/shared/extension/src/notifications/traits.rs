//! The notification seam traits and their verdict types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "sqlx")]
use std::sync::Arc;

use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::{
    Attention, AudienceSubject, Notification, NotificationEvent, RoutePlan, StreamDecl, SubjectRef,
};

#[cfg(feature = "sqlx")]
use super::errors::NotificationSinkError;
use super::errors::{
    ChannelError, NotificationAudienceError, NotificationRouterError, PublishPolicyError,
    VisibilityError,
};

/// Appends a notification event to the outbox on the caller's connection;
/// injected at composition roots as [`DynNotificationSink`].
///
/// A server built without the notifications fabric injects a sink whose
/// `is_available` is `false` and whose `emit` returns
/// [`NotificationSinkError::Unavailable`]; producers check availability
/// before their transaction and skip recording rather than roll back.
///
/// `#[async_trait]` keeps the trait dyn-compatible: producers hold it as
/// `Arc<dyn NotificationSink>`.
#[cfg(feature = "sqlx")]
#[async_trait::async_trait]
pub trait NotificationSink: Send + Sync {
    fn is_available(&self) -> bool;

    async fn emit(
        &self,
        conn: &mut sqlx::PgConnection,
        event: &NotificationEvent,
    ) -> Result<(), NotificationSinkError>;
}

#[cfg(feature = "sqlx")]
pub type DynNotificationSink = Arc<dyn NotificationSink>;

/// Plans the recipients of an event; held as `Arc<dyn NotificationRouter>`.
#[async_trait::async_trait]
pub trait NotificationRouter: Send + Sync {
    fn name(&self) -> &'static str;

    async fn route(&self, event: &NotificationEvent) -> Result<RoutePlan, NotificationRouterError>;
}

/// Whether a user belongs to an audience subject; `Unknown` when the
/// resolver cannot say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Membership {
    Member,
    NotMember,
    Unknown,
}

/// Expands an audience subject to users; held as
/// `Arc<dyn NotificationAudienceResolver>`.
#[async_trait::async_trait]
pub trait NotificationAudienceResolver: Send + Sync {
    fn name(&self) -> &'static str;

    fn handles(&self, subject: &AudienceSubject) -> bool;

    async fn members(
        &self,
        subject: &AudienceSubject,
        limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError>;

    async fn is_member(
        &self,
        subject: &AudienceSubject,
        user: &UserId,
    ) -> Result<Membership, NotificationAudienceError>;
}

/// A publish policy's decision on one publisher and stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishVerdict {
    Admit,
    Refuse { reason: String },
}

/// Decides whether a user may publish to a stream; held as
/// `Arc<dyn PublishPolicy>`.
#[async_trait::async_trait]
pub trait PublishPolicy: Send + Sync {
    fn name(&self) -> &'static str;

    async fn may_publish(
        &self,
        publisher: &UserId,
        stream: &StreamDecl,
    ) -> Result<PublishVerdict, PublishPolicyError>;
}

/// Whether a reader may see a resource, and its excerpt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Hidden,
    Visible { excerpt_allowed: bool },
}

/// Decides per-reader visibility for the resource kinds it names; held as
/// `Arc<dyn ResourceVisibility>`.
#[async_trait::async_trait]
pub trait ResourceVisibility: Send + Sync {
    fn kinds(&self) -> &'static [&'static str];

    async fn visible(
        &self,
        subject: &SubjectRef,
        reader: &UserId,
    ) -> Result<Visibility, VisibilityError>;

    async fn visible_many(
        &self,
        subject: &SubjectRef,
        readers: &[UserId],
    ) -> Result<Vec<Visibility>, VisibilityError> {
        let mut verdicts = Vec::with_capacity(readers.len());
        for reader in readers {
            verdicts.push(self.visible(subject, reader).await?);
        }
        Ok(verdicts)
    }
}

/// Delivers persisted notifications to one surface; held as
/// `Arc<dyn NotificationChannel>`.
#[async_trait::async_trait]
pub trait NotificationChannel: Send + Sync {
    fn surface(&self) -> &'static str;

    fn delivers(&self, attention: Attention) -> bool;

    async fn deliver(&self, notification: &Notification, unread: i64) -> Result<(), ChannelError>;
}
