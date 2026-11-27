//! The handle the shell holds: Slack's status and the Slack-originated items
//! of the reader's inbox, with a change feed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::{Arc, OnceLock};

use sp_notifications::{FeedStatus, Inbox, Notification, NotificationsHandle};
use tokio::sync::watch;

use crate::origin::is_slack_originated;
use crate::status::SlackStatus;

/// Slack as the reader's notifications show it, shared through
/// `SpContext::handles`.
///
/// Cheap to clone. It answers `SlackStatus::Withheld` and no items while the
/// applied instance does not admit the `slack` extension, or while the
/// notifications handle it reads is missing or withheld.
#[derive(Clone)]
pub struct SlackHandle {
    shared: Arc<Shared>,
}

pub(crate) struct Shared {
    admitted: watch::Sender<bool>,
    notifications: OnceLock<NotificationsHandle>,
}

impl Shared {
    pub(crate) fn new() -> Self {
        let (admitted, _) = watch::channel(false);
        Self {
            admitted,
            notifications: OnceLock::new(),
        }
    }

    pub(crate) fn admit(&self, admitted: bool) {
        self.admitted.send_if_modified(|current| {
            let changed = *current != admitted;
            *current = admitted;
            changed
        });
    }

    pub(crate) fn attach(&self, notifications: NotificationsHandle) {
        if self.notifications.set(notifications).is_err() {
            tracing::debug!("slack already reads a notifications handle");
        }
    }

    pub(crate) fn is_attached(&self) -> bool {
        self.notifications.get().is_some()
    }
}

/// The change feed of a [`SlackHandle`]: `changed` resolves when the
/// admission or the inbox changed, and answers `false` once the handle is
/// gone.
#[derive(Debug)]
pub struct SlackChanges {
    admitted: watch::Receiver<bool>,
    inbox: Option<watch::Receiver<Inbox>>,
}

impl SlackChanges {
    pub async fn changed(&mut self) -> bool {
        match &mut self.inbox {
            Some(inbox) => tokio::select! {
                admitted = self.admitted.changed() => admitted.is_ok(),
                inbox = inbox.changed() => inbox.is_ok(),
            },
            None => self.admitted.changed().await.is_ok(),
        }
    }
}

impl SlackHandle {
    pub(crate) const fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }

    pub fn is_admitted(&self) -> bool {
        *self.shared.admitted.borrow()
    }

    pub fn status(&self) -> SlackStatus {
        let Some(inbox) = self.admitted_inbox() else {
            return SlackStatus::Withheld;
        };
        match inbox.status() {
            FeedStatus::Withheld => return SlackStatus::Withheld,
            FeedStatus::Live { .. } => {},
            feed => return SlackStatus::Unknown { feed },
        }
        let mut slack = inbox
            .items()
            .iter()
            .filter(|item| is_slack_originated(item));
        let Some(newest) = slack.next() else {
            return SlackStatus::Quiet;
        };
        let (items, unread) = slack.fold(
            (1, usize::from(newest.read_at.is_none())),
            |(items, unread), item| (items + 1, unread + usize::from(item.read_at.is_none())),
        );
        SlackStatus::Active {
            items,
            unread,
            latest_at: newest.created_at,
        }
    }

    pub fn items(&self) -> Vec<Notification> {
        self.admitted_inbox().map_or_else(Vec::new, |inbox| {
            inbox
                .items()
                .iter()
                .filter(|item| is_slack_originated(item))
                .cloned()
                .collect()
        })
    }

    pub fn subscribe(&self) -> SlackChanges {
        SlackChanges {
            admitted: self.shared.admitted.subscribe(),
            inbox: self
                .shared
                .notifications
                .get()
                .map(NotificationsHandle::subscribe),
        }
    }

    fn admitted_inbox(&self) -> Option<Inbox> {
        if !self.is_admitted() {
            return None;
        }
        self.shared
            .notifications
            .get()
            .map(NotificationsHandle::snapshot)
    }
}

impl fmt::Debug for SlackHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SlackHandle")
            .field("admitted", &self.is_admitted())
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}
