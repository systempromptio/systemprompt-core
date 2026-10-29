//! The per-replica background task that snapshots and checkpoints the
//! sessions this replica holds, follows writes that land on the head
//! outside a session, and evicts idle ones from its cache.
//!
//! Each tick works through the cached sessions a few at a time. An entry's
//! lock is held only to read what is due and to record outcomes, never
//! across a snapshot's blob write or a checkpoint, so one slow session does
//! not hold up the rest or the updates applied to it. A
//! checkpoint another replica is running is left to it and retried on a
//! later tick; a checkpoint that fails or conflicts with the head backs off
//! exponentially; a session closed elsewhere is evicted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use futures_util::stream::{self, StreamExt};

use systemprompt_identifiers::TraceId;
use systemprompt_traits::BackgroundTasks;

use super::entry::{CollabEntry, SharedEntry};
use super::{Checkpoint, CollabError, CollabService};

const TICK_CONCURRENCY: usize = 8;

impl CollabService {
    pub fn start(self: &Arc<Self>, tasks: &BackgroundTasks) {
        let service = Arc::clone(self);
        tasks.spawn_cancellable("document_collab_timer", move |cancel| async move {
            let mut ticks = tokio::time::interval(service.tuning.tick);
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    () = cancel.cancelled() => return,
                    _ = ticks.tick() => service.tick().await,
                }
            }
        });
    }

    pub async fn tick(&self) {
        stream::iter(self.cached_entries())
            .for_each_concurrent(TICK_CONCURRENCY, |(document, entry)| async move {
                if self.tick_entry(&entry).await {
                    self.cache_remove(&document);
                }
            })
            .await;
    }

    async fn tick_entry(&self, shared: &SharedEntry) -> bool {
        let now = Instant::now();
        let session = shared.lock().await.session.id.clone();
        if let Err(error) = self.follow_head(shared, now).await {
            tracing::warn!(session_id = %session, error = %error, "Edit session could not follow the head");
        }
        let (snapshot_due, checkpoint_due) = {
            let entry = shared.lock().await;
            let snapshot_due = entry.unsnapshotted > 0
                && entry
                    .snapshot_due_from
                    .is_some_and(|from| now.duration_since(from) >= self.tuning.snapshot_after);
            let due = entry
                .checkpoint_due_from
                .is_some_and(|from| now.duration_since(from) >= self.tuning.checkpoint_after);
            let backing_off = entry.checkpoint_retry_at.is_some_and(|at| now < at);
            drop(entry);
            (snapshot_due, due && !backing_off)
        };
        if snapshot_due && let Err(error) = self.snapshot(shared).await {
            tracing::warn!(session_id = %session, error = %error, "Edit session snapshot failed");
        }
        if checkpoint_due {
            let outcome = self.checkpoint(shared, &TraceId::system()).await;
            let mut entry = shared.lock().await;
            match outcome {
                Ok(Some(Checkpoint::Unchanged | Checkpoint::Committed(_))) => {
                    entry.checkpoint_failures = 0;
                    entry.checkpoint_retry_at = None;
                },
                Ok(None) => {
                    entry.checkpoint_due_from.get_or_insert(now);
                },
                Err(CollabError::SessionClosed { .. }) => return true,
                Ok(Some(Checkpoint::Conflict { head, .. })) => {
                    self.back_off(&mut entry, now);
                    tracing::warn!(
                        session_id = %session,
                        head_revision_id = %head,
                        failures = entry.checkpoint_failures,
                        "Edit session checkpoint conflicted with the head; backing off"
                    );
                },
                Err(error) => {
                    self.back_off(&mut entry, now);
                    tracing::warn!(
                        session_id = %session,
                        error = %error,
                        failures = entry.checkpoint_failures,
                        "Edit session checkpoint failed; backing off"
                    );
                },
            }
            drop(entry);
        }
        let entry = shared.lock().await;
        let idle = entry.unsnapshotted == 0
            && !entry.snapshotting
            && entry.checkpoint_due_from.is_none()
            && now.duration_since(entry.touched) >= self.tuning.evict_after;
        drop(entry);
        idle
    }

    fn back_off(&self, entry: &mut CollabEntry, now: Instant) {
        entry.checkpoint_failures = entry.checkpoint_failures.saturating_add(1);
        let wait = self
            .tuning
            .checkpoint_after
            .saturating_mul(1 << entry.checkpoint_failures.min(6));
        entry.checkpoint_retry_at = Some(now + wait);
        entry.checkpoint_due_from.get_or_insert(now);
    }
}
