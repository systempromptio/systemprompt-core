//! The feed: pages, capped unread counts, seen and read state, and
//! escalation. Reading a page writes nothing; a surface records what it
//! showed through [`NotificationsService::seen`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{NotificationId, UserId};
use systemprompt_models::notifications::{
    Attention, Escalate, KeysetCursor, MarkRead, NOTIFICATION_ESCALATED, NOTIFICATION_READ,
    Notification, NotificationEscalatedPayload, NotificationPage, NotificationReadPayload,
    ReceiptState, SURFACE_API,
};

use systemprompt_traits::RepositoryError;

use super::NotificationsService;
use crate::error::{NotificationsError, NotificationsResult};
use crate::models::FeedFilter;

const DEFAULT_PAGE: u32 = 50;
const MAX_PAGE: u32 = 100;

/// A feed page request: newest first, after `since` when given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeedQuery {
    pub since: Option<KeysetCursor>,
    pub limit: u32,
    pub attention: Option<Attention>,
    pub unread_only: bool,
}

impl Default for FeedQuery {
    fn default() -> Self {
        Self {
            since: None,
            limit: DEFAULT_PAGE,
            attention: None,
            unread_only: false,
        }
    }
}

/// The ids a mark-read changed and the caller's unread count after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkReadOutcome {
    pub ids: Vec<NotificationId>,
    pub unread: i64,
}

impl NotificationsService {
    pub async fn feed(
        &self,
        user: &UserId,
        query: FeedQuery,
    ) -> NotificationsResult<NotificationPage> {
        let limit = query.limit.clamp(1, MAX_PAGE);
        let notifications = self
            .repos
            .notifications
            .list_feed(
                user,
                FeedFilter {
                    since: query.since,
                    limit: i64::from(limit),
                    attention: query.attention,
                    unread_only: query.unread_only,
                },
            )
            .await?;
        let full_page = u32::try_from(notifications.len()).is_ok_and(|len| len == limit);
        let next_cursor = notifications
            .last()
            .filter(|_| full_page)
            .map(|n| n.id.to_uuid().map(|id| KeysetCursor::new(n.created_at, id)))
            .transpose()
            .map_err(|source| RepositoryError::decode("notifications.id", source))?;
        let (unread, unread_capped) = self.repos.notifications.unread_count(user).await?;
        Ok(NotificationPage {
            notifications,
            next_cursor,
            unread,
            unread_capped,
        })
    }

    pub async fn unread_count(&self, user: &UserId) -> NotificationsResult<i64> {
        Ok(self.unread_state(user).await?.0)
    }

    pub async fn unread_state(&self, user: &UserId) -> NotificationsResult<(i64, bool)> {
        Ok(self.repos.notifications.unread_count(user).await?)
    }

    pub async fn seen(
        &self,
        reader: &UserId,
        ids: &[NotificationId],
    ) -> NotificationsResult<usize> {
        Ok(self
            .repos
            .receipts
            .record_owned(reader, ids, SURFACE_API, ReceiptState::Seen)
            .await?)
    }

    pub async fn mark_read(
        &self,
        user: &UserId,
        request: MarkRead,
        surface: &str,
    ) -> NotificationsResult<MarkReadOutcome> {
        let MarkRead { ids, all } = request;
        let ids = if all {
            self.repos.notifications.mark_all_read(user).await?
        } else {
            self.repos.notifications.mark_read(user, &ids).await?
        };
        self.repos
            .receipts
            .record_detached(&ids, surface, ReceiptState::Read)
            .await?;
        let unread = self.unread_count(user).await?;
        let announced = if all { Vec::new() } else { ids.clone() };
        self.emit(
            user,
            NOTIFICATION_READ,
            &NotificationReadPayload {
                ids: announced,
                unread,
            },
        )
        .await?;
        Ok(MarkReadOutcome { ids, unread })
    }

    pub async fn escalate(
        &self,
        user: &UserId,
        id: &NotificationId,
        request: Escalate,
        surface: &str,
    ) -> NotificationsResult<Notification> {
        let Escalate { note } = request;
        let current = self
            .repos
            .notifications
            .find_pooled(user, id)
            .await?
            .ok_or_else(|| NotificationsError::NotFound { id: id.clone() })?;
        if current.attention != Attention::Agent {
            return Err(NotificationsError::NotEscalatable {
                id: id.clone(),
                attention: current.attention,
            });
        }
        let escalated = self
            .repos
            .notifications
            .escalate(user, id, note.as_deref())
            .await?
            .ok_or_else(|| NotificationsError::NotEscalatable {
                id: id.clone(),
                attention: current.attention,
            })?;
        self.repos
            .receipts
            .record_detached(std::slice::from_ref(id), surface, ReceiptState::Escalated)
            .await?;
        self.emit(
            user,
            NOTIFICATION_ESCALATED,
            &NotificationEscalatedPayload {
                id: id.clone(),
                attention: escalated.attention,
            },
        )
        .await?;
        self.pipeline
            .deliver(std::slice::from_ref(&escalated.id))
            .await;
        Ok(escalated)
    }
}
