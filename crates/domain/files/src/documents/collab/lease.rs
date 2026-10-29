//! The checkpoint lease of a live edit session: claiming it, renewing it
//! while a checkpoint runs, and releasing it.
//!
//! A claim waits for a lease another replica holds only when asked to, and
//! for a bounded time. A checkpoint runs fenced by its lease: a heartbeat
//! renews the lease well before it lapses, and a renewal that finds the
//! lease gone, or that cannot reach the database before the lease would
//! lapse, ends the checkpoint with [`CollabError::CheckpointLeaseLost`]
//! before it records anything. The heartbeat runs inside the checkpoint's
//! own future rather than as a spawned task, so it can never outlive the
//! work it fences.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::time::{Duration, Instant};

use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, TraceId};

use super::entry::CollabEntry;
use super::{CollabError, CollabService};
use crate::documents::models::DocumentEditSession;
use crate::documents::repository::{CheckpointClaim, CheckpointLease};

const CHECKPOINT_LEASE: Duration = Duration::from_secs(60);
const LEASE_RENEW: Duration = Duration::from_secs(20);
const LEASE_WAIT: Duration = Duration::from_secs(5);
const LEASE_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClaimWait {
    No,
    Bounded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettledBy {
    Participant,
    Reaper,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Fence<'a> {
    pub(super) key: &'a SessionKey,
    pub(super) lease: &'a CheckpointLease,
    pub(super) trace: &'a TraceId,
    pub(super) by: SettledBy,
}

#[derive(Debug, Clone)]
pub(super) struct SessionKey {
    pub(super) document: DocumentId,
    pub(super) session: DocumentEditSessionId,
}

impl SessionKey {
    pub(super) fn of(entry: &CollabEntry) -> Self {
        Self {
            document: entry.session.document_id.clone(),
            session: entry.session.id.clone(),
        }
    }

    pub(super) fn lease_lost(&self) -> CollabError {
        CollabError::CheckpointLeaseLost {
            document_id: self.document.clone(),
            session_id: self.session.clone(),
        }
    }

    pub(super) fn closed(&self) -> CollabError {
        CollabError::SessionClosed {
            document_id: self.document.clone(),
            session_id: self.session.clone(),
        }
    }
}

impl CollabService {
    pub(super) async fn claim_row(
        &self,
        key: &SessionKey,
        lease: &CheckpointLease,
        wait: ClaimWait,
    ) -> Result<Option<DocumentEditSession>, CollabError> {
        let sessions = &self.documents.repositories().edit_sessions;
        let deadline = Instant::now() + LEASE_WAIT;
        loop {
            match sessions
                .claim_checkpoint(&key.session, lease, CHECKPOINT_LEASE)
                .await?
            {
                CheckpointClaim::Claimed(session) => return Ok(Some(session)),
                CheckpointClaim::Closed => return Err(key.closed()),
                CheckpointClaim::Busy => match wait {
                    ClaimWait::No => return Ok(None),
                    ClaimWait::Bounded if Instant::now() < deadline => {
                        tokio::time::sleep(LEASE_POLL).await;
                    },
                    ClaimWait::Bounded => {
                        return Err(CollabError::CheckpointBusy {
                            document_id: key.document.clone(),
                            session_id: key.session.clone(),
                        });
                    },
                },
            }
        }
    }

    pub(super) async fn fenced<T>(
        &self,
        key: &SessionKey,
        lease: &CheckpointLease,
        work: impl Future<Output = Result<T, CollabError>>,
    ) -> Result<T, CollabError> {
        let sessions = &self.documents.repositories().edit_sessions;
        let heartbeat = async {
            let mut held_since = Instant::now();
            let mut ticks = tokio::time::interval(LEASE_RENEW);
            ticks.tick().await;
            loop {
                ticks.tick().await;
                let attempt = Instant::now();
                match sessions
                    .renew_checkpoint(&key.session, lease, CHECKPOINT_LEASE)
                    .await
                {
                    Ok(true) => held_since = attempt,
                    Ok(false) => return key.lease_lost(),
                    Err(error) if held_since.elapsed() + LEASE_RENEW >= CHECKPOINT_LEASE => {
                        tracing::warn!(
                            session_id = %key.session,
                            error = %error,
                            "Edit session checkpoint lease could not be renewed before it lapses"
                        );
                        return key.lease_lost();
                    },
                    Err(error) => tracing::warn!(
                        session_id = %key.session,
                        error = %error,
                        "Edit session checkpoint lease not renewed; retrying before it lapses"
                    ),
                }
            }
        };
        tokio::select! {
            outcome = work => outcome,
            lost = heartbeat => {
                tracing::warn!(
                    session_id = %key.session,
                    "Edit session checkpoint abandoned: its lease was lost while it ran"
                );
                Err(lost)
            },
        }
    }

    pub(super) async fn release(
        &self,
        session_id: &DocumentEditSessionId,
        lease: &CheckpointLease,
    ) {
        if let Err(error) = self
            .documents
            .repositories()
            .edit_sessions
            .release_checkpoint(session_id, lease)
            .await
        {
            tracing::warn!(
                session_id = %session_id,
                error = %error,
                "Edit session checkpoint lease not released; it lapses when it expires"
            );
        }
    }
}
