//! The persisted inbox: newest first, bounded, written atomically beside the
//! bridge's other metadata.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::{
    Attention, AttentionReason, KeysetCursor, Notification, NotificationPage,
};

pub const MAX_ITEMS: usize = 200;
pub const INBOX_FILE_NAME: &str = "inbox.json";

/// Why the inbox file could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum InboxStateError {
    #[error("inbox file {path} could not be read")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("inbox file {path} could not be written")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("inbox file {path} is not an inbox")]
    Decode {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("inbox could not be encoded")]
    Encode(#[source] serde_json::Error),
}

/// The reader's inbox as last seen.
///
/// `toasted_through` is the newest `created_at` already raised as a toast, so
/// a catch-up never toasts twice. `older_cursor` points below the oldest held
/// item and is read only by an explicit load-older request; catch-up always
/// starts from the newest item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxState {
    pub unread: i64,
    #[serde(default)]
    pub unread_capped: bool,
    #[serde(default)]
    pub older_cursor: Option<KeysetCursor>,
    pub items: Vec<Notification>,
    #[serde(default)]
    pub toasted_through: Option<DateTime<Utc>>,
}

/// What one newest-first page did to the inbox: the ids it added, how many
/// items it carried, whether it overlapped what the inbox already held, and
/// the cursor below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMerge {
    pub fresh: Vec<NotificationId>,
    pub fetched: usize,
    pub reached_known: bool,
    pub next_cursor: Option<KeysetCursor>,
}

#[must_use]
pub fn inbox_path() -> Option<PathBuf> {
    crate::config::paths::bridge_metadata_dir().map(|dir| dir.join(INBOX_FILE_NAME))
}

impl InboxState {
    pub fn load(path: &Path) -> Result<Self, InboxStateError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            },
            Err(source) => {
                return Err(InboxStateError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            },
        };
        serde_json::from_slice(&bytes).map_err(|source| InboxStateError::Decode {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), InboxStateError> {
        let bytes = serde_json::to_vec(self).map_err(InboxStateError::Encode)?;
        crate::fsutil::atomic_write_0600(path, &bytes).map_err(|source| InboxStateError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    #[must_use]
    pub fn find(&self, id: &NotificationId) -> Option<&Notification> {
        self.items.iter().find(|item| &item.id == id)
    }

    pub fn upsert(&mut self, notification: Notification) -> bool {
        let existing = self
            .items
            .iter_mut()
            .find(|item| item.id == notification.id);
        let fresh = existing.is_none();
        if let Some(existing) = existing {
            *existing = notification;
        } else {
            self.items.push(notification);
        }
        self.items.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        self.items.truncate(MAX_ITEMS);
        fresh
    }

    pub fn merge_newest(&mut self, page: NotificationPage) -> PageMerge {
        let reached_known = page
            .notifications
            .iter()
            .any(|item| self.find(&item.id).is_some());
        let fetched = page.notifications.len();
        let fresh = self.merge_items(page.notifications);
        self.unread = page.unread;
        self.unread_capped = page.unread_capped;
        PageMerge {
            fresh,
            fetched,
            reached_known,
            next_cursor: page.next_cursor,
        }
    }

    pub fn merge_older(&mut self, page: NotificationPage) -> Vec<NotificationId> {
        let fresh = self.merge_items(page.notifications);
        self.unread = page.unread;
        self.unread_capped = page.unread_capped;
        self.older_cursor = page.next_cursor;
        fresh
    }

    fn merge_items(&mut self, items: Vec<Notification>) -> Vec<NotificationId> {
        let mut fresh = Vec::new();
        for notification in items {
            let id = notification.id.clone();
            if self.upsert(notification) {
                fresh.push(id);
            }
        }
        fresh
    }

    pub fn apply_read(&mut self, ids: &[NotificationId], unread: i64, at: DateTime<Utc>) {
        for item in &mut self.items {
            if item.read_at.is_none() && (ids.is_empty() || ids.contains(&item.id)) {
                item.read_at = Some(at);
            }
        }
        self.unread = unread;
    }

    pub fn apply_escalated(&mut self, id: &NotificationId, attention: Attention) -> bool {
        let Some(item) = self.items.iter_mut().find(|item| &item.id == id) else {
            return false;
        };
        if item.attention == attention {
            return false;
        }
        item.escalated_from = Some(item.attention);
        item.attention = attention;
        item.attention_reason = AttentionReason::Escalated;
        true
    }
}
