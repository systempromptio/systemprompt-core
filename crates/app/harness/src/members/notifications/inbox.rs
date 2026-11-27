//! The inbox as last fetched: newest first, bounded, with the unread count the
//! instance reported and the state of the feed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, Utc};
use systemprompt_device::view::SyncErrorKind;
use systemprompt_identifiers::NotificationId;
use systemprompt_models::api::KeysetCursor;
use systemprompt_models::notifications::{Notification, NotificationPage};

pub const INBOX_CAPACITY: usize = 200;

/// The unread count as the instance reported it; `capped` means the
/// instance stopped counting and the true count is higher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnreadCount {
    pub count: u64,
    pub capped: bool,
}

impl UnreadCount {
    pub const fn from_wire(unread: i64, capped: bool) -> Self {
        Self {
            count: if unread > 0 { unread.unsigned_abs() } else { 0 },
            capped,
        }
    }
}

/// Where the feed stands.
///
/// `Withheld` while no instance is applied or it does not enable
/// notifications; `Connecting` from the moment an instance is followed to its
/// first answer; `NotServed` when the instance has no notifications feed;
/// `Failing` after a poll failed, until the retry after `retry_in`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedStatus {
    Withheld,
    Connecting,
    Live {
        refreshed_at: DateTime<Utc>,
    },
    NotServed,
    Failing {
        kind: SyncErrorKind,
        retry_in: Duration,
    },
}

/// The reader's inbox: at most [`INBOX_CAPACITY`] notifications, newest first
/// by creation time, the unread count and the feed's status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbox {
    status: FeedStatus,
    items: Vec<Notification>,
    unread: UnreadCount,
    end_reached: bool,
    generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageMerge {
    pub(crate) fetched: usize,
    pub(crate) reached_known: bool,
    pub(crate) next_cursor: Option<KeysetCursor>,
}

impl Inbox {
    pub(crate) const fn withheld(generation: u64) -> Self {
        Self::empty(FeedStatus::Withheld, generation)
    }

    pub(crate) const fn connecting(generation: u64) -> Self {
        Self::empty(FeedStatus::Connecting, generation)
    }

    const fn empty(status: FeedStatus, generation: u64) -> Self {
        Self {
            status,
            items: Vec::new(),
            unread: UnreadCount {
                count: 0,
                capped: false,
            },
            end_reached: false,
            generation,
        }
    }

    pub const fn status(&self) -> FeedStatus {
        self.status
    }

    pub fn items(&self) -> &[Notification] {
        &self.items
    }

    pub const fn unread(&self) -> UnreadCount {
        self.unread
    }

    pub fn find(&self, id: &NotificationId) -> Option<&Notification> {
        self.items.iter().find(|item| item.id == *id)
    }

    pub fn older_cursor(&self) -> Option<KeysetCursor> {
        if self.end_reached {
            return None;
        }
        let oldest = self.items.last()?;
        let id = oldest.id.to_uuid().ok()?;
        Some(KeysetCursor::new(oldest.created_at, id))
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) const fn set_status(&mut self, status: FeedStatus) {
        self.status = status;
    }

    pub(crate) fn merge_newest(&mut self, page: NotificationPage) -> PageMerge {
        let fetched = page.notifications.len();
        let reached_known =
            fetched > 0 && page.notifications.iter().all(|item| self.holds(&item.id));
        let next_cursor = page.next_cursor;
        self.unread = UnreadCount::from_wire(page.unread, page.unread_capped);
        self.upsert_all(page.notifications);
        PageMerge {
            fetched,
            reached_known,
            next_cursor,
        }
    }

    pub(crate) fn merge_older(&mut self, page: NotificationPage) {
        self.unread = UnreadCount::from_wire(page.unread, page.unread_capped);
        let exhausted = page.next_cursor.is_none();
        self.upsert_all(page.notifications);
        if exhausted && self.items.len() < INBOX_CAPACITY {
            self.end_reached = true;
        }
    }

    pub(crate) const fn mark_end_reached(&mut self) {
        if self.items.len() < INBOX_CAPACITY {
            self.end_reached = true;
        }
    }

    pub(crate) fn apply_read(&mut self, ids: &[NotificationId], unread: i64, at: DateTime<Utc>) {
        for item in &mut self.items {
            if item.read_at.is_none() && ids.contains(&item.id) {
                item.read_at = Some(at);
            }
        }
        self.unread = UnreadCount::from_wire(unread, false);
    }

    fn holds(&self, id: &NotificationId) -> bool {
        self.items.iter().any(|item| item.id == *id)
    }

    fn upsert_all(&mut self, incoming: Vec<Notification>) {
        for notification in incoming {
            match self
                .items
                .iter_mut()
                .find(|item| item.id == notification.id)
            {
                Some(held) => *held = notification,
                None => self.items.push(notification),
            }
        }
        self.items.sort_by(|left, right| {
            (right.created_at, right.id.as_str()).cmp(&(left.created_at, left.id.as_str()))
        });
        if self.items.len() > INBOX_CAPACITY {
            self.items.truncate(INBOX_CAPACITY);
            self.end_reached = false;
        }
    }
}
