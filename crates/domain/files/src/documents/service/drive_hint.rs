//! The `document.drive.changed` hint: after a revision lands, every user who
//! can read the document is told, with no ids, that their Drive changed, so
//! a client re-reads the access-filtered change feed instead of waiting for
//! its next poll.
//!
//! Recipients are the document's access holders, resolved when the hint is
//! sent. Any repository or resolver error withholds the hint from everyone:
//! the client's poll covers it, and no partial or wider audience is ever
//! addressed.
//!
//! Hints are coalesced per document on each replica: the first revision in
//! a quiet document sends one at once, and every later revision inside
//! [`DRIVE_HINT_INTERVAL`] folds into one trailing hint sent when that
//! window closes, which opens the next window. Both run on the injected
//! `BackgroundTasks`, so a commit never waits on recipient resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use systemprompt_identifiers::{DocumentId, UserId};

use super::DocumentService;
use crate::documents::models::Document;

pub const DRIVE_HINT_INTERVAL: Duration = Duration::from_secs(2);

const DRIVE_HINT_PRUNE_AT: usize = 4096;

#[derive(Debug, Clone)]
struct PendingHint {
    document: Document,
    actor: UserId,
}

#[derive(Debug)]
struct HintWindow {
    until: Instant,
    trailing: Option<PendingHint>,
}

#[derive(Debug, PartialEq, Eq)]
enum Admission {
    Now,
    Trailing(Instant),
    Coalesced,
}

#[derive(Debug, Default)]
pub(super) struct DriveHintGate {
    windows: Mutex<HashMap<DocumentId, HintWindow>>,
}

impl DriveHintGate {
    fn admit(&self, document: &Document, actor: &UserId, now: Instant) -> Admission {
        let pending = PendingHint {
            document: document.clone(),
            actor: actor.clone(),
        };
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        if windows.len() >= DRIVE_HINT_PRUNE_AT {
            windows.retain(|_, window| window.until > now || window.trailing.is_some());
        }
        let admission = match windows.get_mut(&document.id) {
            Some(window) if now < window.until => {
                if window.trailing.replace(pending).is_some() {
                    Admission::Coalesced
                } else {
                    Admission::Trailing(window.until)
                }
            },
            _ => {
                windows.insert(
                    document.id.clone(),
                    HintWindow {
                        until: now + DRIVE_HINT_INTERVAL,
                        trailing: None,
                    },
                );
                Admission::Now
            },
        };
        drop(windows);
        admission
    }

    fn take_trailing(&self, document: &DocumentId, now: Instant) -> Option<PendingHint> {
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        let pending = windows.get_mut(document).and_then(|window| {
            let pending = window.trailing.take()?;
            window.until = now + DRIVE_HINT_INTERVAL;
            Some(pending)
        });
        drop(windows);
        pending
    }
}

impl DocumentService {
    pub(super) fn hint_drive_changed(&self, document: &Document, actor: &UserId) {
        match self.drive_hints.admit(document, actor, Instant::now()) {
            Admission::Coalesced => {},
            Admission::Now => {
                let service = self.clone();
                let pending = PendingHint {
                    document: document.clone(),
                    actor: actor.clone(),
                };
                self.tasks.spawn("document_drive_hint", async move {
                    service.send_drive_hint(&pending).await;
                });
            },
            Admission::Trailing(at) => {
                let service = self.clone();
                let id = document.id.clone();
                self.tasks.spawn_cancellable(
                    "document_drive_hint_trailing",
                    move |cancel| async move {
                        tokio::select! {
                            () = cancel.cancelled() => return,
                            () = tokio::time::sleep_until(at.into()) => {},
                        }
                        if let Some(pending) =
                            service.drive_hints.take_trailing(&id, Instant::now())
                        {
                            service.send_drive_hint(&pending).await;
                        }
                    },
                );
            },
        }
    }

    async fn send_drive_hint(&self, pending: &PendingHint) {
        let document = &pending.document;
        let recipients = match self.access_holders(document).await {
            Ok(recipients) => recipients,
            Err(error) => {
                tracing::warn!(
                    document_id = %document.id,
                    error = %error,
                    "Drive-change hint withheld: its recipients could not be resolved"
                );
                return;
            },
        };
        if recipients.is_empty() {
            return;
        }
        if let Err(error) = self.events.drive_changed(&pending.actor, &recipients).await {
            tracing::warn!(
                document_id = %document.id,
                recipients = recipients.len(),
                error = %error,
                "Drive-change hint delivered locally but not relayed to other replicas"
            );
        }
    }
}
