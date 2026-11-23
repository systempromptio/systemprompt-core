//! The `document.drive.changed` hint: after a change to what a Drive shows,
//! every user it concerns is told, with no ids, that their Drive changed, so
//! a client re-reads the access-filtered change feed instead of waiting for
//! its next poll.
//!
//! A hint follows a landed revision, a document's rename, move, trash and
//! restore, a grant written or removed, an ownership transfer, and a
//! folder's creation, rename, move and deletion. A grant that expires sends
//! none; the client's poll covers it.
//!
//! Recipients are resolved when the hint is sent (see `drive_audience`),
//! joined with the users who held access before a change that takes it
//! away, and capped at the subject fan-out limit. Any repository or
//! resolver error, before the change or at sending, withholds the hint from
//! everyone: the client's poll covers it, and no partial audience is ever
//! addressed.
//!
//! Hints are coalesced per document or folder on each replica: the first
//! change to a quiet one sends a hint at once, and every later change inside
//! [`DRIVE_HINT_INTERVAL`] folds into one trailing hint sent when that
//! window closes, which opens the next window. The trailing hint takes the
//! scope of the last change and the former holders of all of them. Both run
//! on the injected `BackgroundTasks`, so a write never waits on recipient
//! resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use systemprompt_identifiers::{DocumentFolderId, DocumentId, UserId};

use super::DocumentService;
use crate::documents::access::Granted;
use crate::documents::models::Document;

pub const DRIVE_HINT_INTERVAL: Duration = Duration::from_secs(2);

const DRIVE_HINT_PRUNE_AT: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum HintKey {
    Document(DocumentId),
    Folder(DocumentFolderId),
}

#[derive(Debug, Clone)]
pub(super) enum HintScope {
    Document(Box<Document>),
    Folder(DocumentFolderId),
    Former,
}

#[derive(Debug, Clone)]
pub(super) struct FormerHolders(pub(super) Option<BTreeSet<UserId>>);

#[derive(Debug, Clone)]
pub(super) struct DriveHint {
    pub(super) key: HintKey,
    pub(super) scope: HintScope,
    pub(super) actor: UserId,
    pub(super) also: BTreeSet<UserId>,
}

impl DriveHint {
    pub(super) fn document(document: &Document, actor: &UserId) -> Self {
        Self {
            key: HintKey::Document(document.id.clone()),
            scope: HintScope::Document(Box::new(document.clone())),
            actor: actor.clone(),
            also: BTreeSet::new(),
        }
    }

    pub(super) fn folder(folder: &DocumentFolderId, actor: &UserId) -> Self {
        Self {
            key: HintKey::Folder(folder.clone()),
            scope: HintScope::Folder(folder.clone()),
            actor: actor.clone(),
            also: BTreeSet::new(),
        }
    }

    pub(super) fn of(granted: &Granted, actor: &UserId) -> Option<Self> {
        match (&granted.document, &granted.folder) {
            (Some(document), _) => Some(Self::document(document, actor)),
            (None, Some(folder)) => Some(Self::folder(&folder.id, actor)),
            (None, None) => None,
        }
    }

    pub(super) fn with(mut self, before: FormerHolders) -> Option<Self> {
        self.also.extend(before.0?);
        Some(self)
    }

    pub(super) fn former(mut self, before: FormerHolders) -> Option<Self> {
        self.scope = HintScope::Former;
        self.with(before)
    }
}

#[derive(Debug)]
struct HintWindow {
    until: Instant,
    trailing: Option<DriveHint>,
}

#[derive(Debug)]
enum Admission {
    Now(DriveHint),
    Trailing(Instant),
    Coalesced,
}

#[derive(Debug, Default)]
pub(super) struct DriveHintGate {
    windows: Mutex<HashMap<HintKey, HintWindow>>,
}

impl DriveHintGate {
    fn admit(&self, mut hint: DriveHint, now: Instant) -> Admission {
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        if windows.len() >= DRIVE_HINT_PRUNE_AT {
            windows.retain(|_, window| window.until > now || window.trailing.is_some());
        }
        let admission = match windows.get_mut(&hint.key) {
            Some(window) if now < window.until => match window.trailing.take() {
                Some(earlier) => {
                    hint.also.extend(earlier.also);
                    window.trailing = Some(hint);
                    Admission::Coalesced
                },
                None => {
                    window.trailing = Some(hint);
                    Admission::Trailing(window.until)
                },
            },
            _ => {
                windows.insert(
                    hint.key.clone(),
                    HintWindow {
                        until: now + DRIVE_HINT_INTERVAL,
                        trailing: None,
                    },
                );
                Admission::Now(hint)
            },
        };
        drop(windows);
        admission
    }

    fn take_trailing(&self, key: &HintKey, now: Instant) -> Option<DriveHint> {
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        let pending = windows.get_mut(key).and_then(|window| {
            let pending = window.trailing.take()?;
            window.until = now + DRIVE_HINT_INTERVAL;
            Some(pending)
        });
        drop(windows);
        pending
    }
}

impl DocumentService {
    pub(super) fn hint_drive_changed(&self, hint: impl Into<Option<DriveHint>>) {
        let Some(hint) = hint.into() else {
            return;
        };
        let key = hint.key.clone();
        match self.drive_hints.admit(hint, Instant::now()) {
            Admission::Coalesced => {},
            Admission::Now(hint) => {
                let service = self.clone();
                self.tasks.spawn("document_drive_hint", async move {
                    service.send_drive_hint(&hint).await;
                });
            },
            Admission::Trailing(at) => {
                let service = self.clone();
                self.tasks.spawn_cancellable(
                    "document_drive_hint_trailing",
                    move |cancel| async move {
                        tokio::select! {
                            () = cancel.cancelled() => return,
                            () = tokio::time::sleep_until(at.into()) => {},
                        }
                        if let Some(hint) = service.drive_hints.take_trailing(&key, Instant::now())
                        {
                            service.send_drive_hint(&hint).await;
                        }
                    },
                );
            },
        }
    }

    async fn send_drive_hint(&self, hint: &DriveHint) {
        let recipients = match self.hint_audience(hint).await {
            Ok(recipients) => recipients,
            Err(error) => {
                tracing::warn!(
                    target_id = ?hint.key,
                    error = %error,
                    "Drive-change hint withheld: its recipients could not be resolved"
                );
                return;
            },
        };
        if recipients.is_empty() {
            return;
        }
        if let Err(error) = self.events.drive_changed(&hint.actor, &recipients).await {
            tracing::warn!(
                target_id = ?hint.key,
                recipients = recipients.len(),
                error = %error,
                "Drive-change hint delivered locally but not relayed to other replicas"
            );
        }
    }
}
