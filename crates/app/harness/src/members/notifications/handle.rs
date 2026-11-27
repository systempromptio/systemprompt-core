//! The handle the shell and other extensions hold: the inbox as last fetched,
//! its change feed, and the reader's actions on it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use chrono::Utc;
use sp_protocol::ids::{AccountId, NotificationId};
use sp_protocol::wire::notifications::{MarkRead, MarkSeen};
use tokio::sync::futures::Notified;
use tokio::sync::{Notify, watch};

use crate::members::notifications::client::{FeedQuery, NotificationsClient};
use crate::members::notifications::error::NotificationsError;
use crate::members::notifications::feed::FEED_PAGE;
use crate::members::notifications::inbox::{FeedStatus, Inbox, UnreadCount};

/// The reader's notifications, shared through `SpContext::handles`.
///
/// Cheap to clone; every clone sees one inbox. `follow` points it at an
/// account's instance and `withhold` empties it and refuses every action
/// (the extension's manifest applier calls both); the feed service fills it.
/// `subscribe` is the change feed. An answer that arrives after the handle
/// moved to another account, or was withheld, is dropped.
#[derive(Clone)]
pub struct NotificationsHandle {
    shared: Arc<Shared>,
}

struct Shared {
    target: watch::Sender<Option<Target>>,
    inbox: watch::Sender<Inbox>,
    refresh: Notify,
}

#[derive(Clone)]
pub(crate) struct Target {
    pub(crate) account: AccountId,
    pub(crate) client: NotificationsClient,
    pub(crate) generation: u64,
}

impl NotificationsHandle {
    pub fn new() -> Self {
        let (target, _) = watch::channel(None);
        let (inbox, _) = watch::channel(Inbox::withheld(0));
        Self {
            shared: Arc::new(Shared {
                target,
                inbox,
                refresh: Notify::new(),
            }),
        }
    }

    pub fn follow(&self, account: AccountId, client: NotificationsClient) {
        let same = self
            .shared
            .target
            .borrow()
            .as_ref()
            .is_some_and(|held| held.account == account && held.client.base() == client.base());
        if same {
            self.shared.target.send_if_modified(|current| {
                if let Some(held) = current {
                    held.client = client;
                }
                false
            });
            return;
        }
        let generation = self.next_generation();
        self.shared
            .inbox
            .send_replace(Inbox::connecting(generation));
        self.shared.target.send_replace(Some(Target {
            account,
            client,
            generation,
        }));
    }

    pub fn withhold(&self) {
        let withheld = self.shared.target.borrow().is_none()
            && self.shared.inbox.borrow().status() == FeedStatus::Withheld;
        if withheld {
            return;
        }
        let generation = self.next_generation();
        self.shared.target.send_replace(None);
        self.shared.inbox.send_replace(Inbox::withheld(generation));
    }

    pub fn snapshot(&self) -> Inbox {
        self.shared.inbox.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Inbox> {
        self.shared.inbox.subscribe()
    }

    pub fn status(&self) -> FeedStatus {
        self.shared.inbox.borrow().status()
    }

    pub fn unread(&self) -> UnreadCount {
        self.shared.inbox.borrow().unread()
    }

    pub fn refresh(&self) {
        self.shared.refresh.notify_one();
    }

    pub async fn mark_read(&self, ids: Vec<NotificationId>) -> Result<(), NotificationsError> {
        if ids.is_empty() {
            return Ok(());
        }
        self.read(MarkRead { ids, all: false }).await
    }

    pub async fn mark_all_read(&self) -> Result<(), NotificationsError> {
        self.read(MarkRead {
            ids: Vec::new(),
            all: true,
        })
        .await
    }

    pub async fn mark_seen(&self, ids: Vec<NotificationId>) -> Result<u64, NotificationsError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let target = self.target()?;
        let answer = target.client.mark_seen(&MarkSeen { ids }).await?;
        Ok(answer.seen)
    }

    pub async fn load_older(&self) -> Result<(), NotificationsError> {
        let target = self.target()?;
        let Some(cursor) = self.shared.inbox.borrow().older_cursor() else {
            return Ok(());
        };
        let query = FeedQuery::newest(FEED_PAGE).since(cursor);
        let page = target.client.feed(&query).await?;
        self.update(target.generation, |inbox| inbox.merge_older(page));
        Ok(())
    }

    pub(crate) fn targets(&self) -> watch::Receiver<Option<Target>> {
        self.shared.target.subscribe()
    }

    pub(crate) fn refresh_requested(&self) -> Notified<'_> {
        self.shared.refresh.notified()
    }

    pub(crate) fn update<R>(
        &self,
        generation: u64,
        change: impl FnOnce(&mut Inbox) -> R,
    ) -> Option<R> {
        let mut outcome = None;
        self.shared.inbox.send_if_modified(|inbox| {
            if inbox.generation() != generation {
                return false;
            }
            outcome = Some(change(inbox));
            true
        });
        outcome
    }

    async fn read(&self, request: MarkRead) -> Result<(), NotificationsError> {
        let target = self.target()?;
        let answer = target.client.mark_read(&request).await?;
        self.update(target.generation, |inbox| {
            inbox.apply_read(&answer.ids, answer.unread, Utc::now());
        });
        Ok(())
    }

    fn target(&self) -> Result<Target, NotificationsError> {
        self.shared
            .target
            .borrow()
            .clone()
            .ok_or(NotificationsError::Withheld)
    }

    fn next_generation(&self) -> u64 {
        self.shared.inbox.borrow().generation().wrapping_add(1)
    }
}

impl Default for NotificationsHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for NotificationsHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotificationsHandle")
            .field("status", &self.status())
            .field("unread", &self.unread())
            .finish_non_exhaustive()
    }
}
