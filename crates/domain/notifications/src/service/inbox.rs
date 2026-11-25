//! [`NotificationInbox`] over the service: the feed other capabilities'
//! surfaces work, with every service error classified for the caller.
//! [`SlotInbox`] is the provided handle: it reads the late-bound service on
//! each call and answers `Unavailable` until it is bound.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{InboxError, NotificationInbox};
use systemprompt_identifiers::{NotificationId, UserId};
use systemprompt_models::notifications::{
    Escalate, FeedQuery, MarkRead, MarkReadOutcome, Notification, NotificationPage,
};
use systemprompt_traits::RepositoryError;

use super::{NotificationsService, NotificationsServiceSlot};
use crate::error::NotificationsError;

fn classify(error: NotificationsError) -> InboxError {
    match error {
        NotificationsError::NotFound { .. }
        | NotificationsError::SubscriptionNotFound { .. }
        | NotificationsError::StreamNotFound { .. } => InboxError::NotFound(Box::new(error)),
        NotificationsError::Repository(repo @ RepositoryError::NotFound { .. }) => {
            InboxError::NotFound(Box::new(repo))
        },
        NotificationsError::PublishRefused { .. } => InboxError::Forbidden(Box::new(error)),
        NotificationsError::NotEscalatable { .. }
        | NotificationsError::InvalidDeclaration(_)
        | NotificationsError::Kind(_) => InboxError::InvalidParams(Box::new(error)),
        NotificationsError::Repository(
            repo @ (RepositoryError::Conflict { .. }
            | RepositoryError::InvalidArgument { .. }
            | RepositoryError::InvalidData { .. }),
        ) => InboxError::InvalidParams(Box::new(repo)),
        other => InboxError::Failed(Box::new(other)),
    }
}

#[async_trait::async_trait]
impl NotificationInbox for NotificationsService {
    async fn feed(&self, user: &UserId, query: FeedQuery) -> Result<NotificationPage, InboxError> {
        Self::feed(self, user, query).await.map_err(classify)
    }

    async fn mark_read(
        &self,
        user: &UserId,
        mark: MarkRead,
        surface: &'static str,
    ) -> Result<MarkReadOutcome, InboxError> {
        Self::mark_read(self, user, mark, surface)
            .await
            .map_err(classify)
    }

    async fn escalate(
        &self,
        user: &UserId,
        id: &NotificationId,
        escalate: Escalate,
        surface: &'static str,
    ) -> Result<Notification, InboxError> {
        Self::escalate(self, user, id, escalate, surface)
            .await
            .map_err(classify)
    }
}

/// The provided inbox: the service the notifications slot binds, read per call.
#[derive(Debug, Clone)]
pub struct SlotInbox(pub(crate) Arc<NotificationsServiceSlot>);

impl SlotInbox {
    fn service(&self) -> Result<&NotificationsService, InboxError> {
        self.0.get().map(AsRef::as_ref).ok_or(InboxError::Unavailable)
    }
}

#[async_trait::async_trait]
impl NotificationInbox for SlotInbox {
    async fn feed(&self, user: &UserId, query: FeedQuery) -> Result<NotificationPage, InboxError> {
        NotificationInbox::feed(self.service()?, user, query).await
    }

    async fn mark_read(
        &self,
        user: &UserId,
        mark: MarkRead,
        surface: &'static str,
    ) -> Result<MarkReadOutcome, InboxError> {
        NotificationInbox::mark_read(self.service()?, user, mark, surface).await
    }

    async fn escalate(
        &self,
        user: &UserId,
        id: &NotificationId,
        escalate: Escalate,
        surface: &'static str,
    ) -> Result<Notification, InboxError> {
        NotificationInbox::escalate(self.service()?, user, id, escalate, surface).await
    }
}
