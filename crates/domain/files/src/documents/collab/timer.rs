//! The per-replica background task that snapshots and checkpoints the
//! sessions this replica holds, and evicts idle ones from its cache.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use systemprompt_identifiers::TraceId;
use systemprompt_traits::BackgroundTasks;

use super::CollabService;
use super::entry::SharedEntry;

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
        for (document, entry) in self.cached_entries() {
            if self.tick_entry(&entry).await {
                self.cache_remove(&document);
            }
        }
    }

    async fn tick_entry(&self, entry: &SharedEntry) -> bool {
        let mut entry = entry.lock().await;
        let now = Instant::now();
        let session = entry.session.id.clone();
        let snapshot_due = entry.unsnapshotted > 0
            && entry
                .snapshot_due_from
                .is_some_and(|from| now.duration_since(from) >= self.tuning.snapshot_after);
        if snapshot_due && let Err(error) = self.snapshot(&mut entry).await {
            tracing::warn!(session_id = %session, error = %error, "Edit session snapshot failed");
        }
        // Why: core lands MCP and API writes on the head without touching the
        // live session, so each tick asks the head whether it moved (one row
        // read per open document) and pulls the change in when it did.
        if let Err(error) = self.follow_head(&mut entry, now).await {
            tracing::warn!(session_id = %session, error = %error, "Edit session could not follow the head");
        }
        let checkpoint_due = entry
            .checkpoint_due_from
            .is_some_and(|from| now.duration_since(from) >= self.tuning.checkpoint_after);
        let backing_off = entry.checkpoint_retry_at.is_some_and(|at| now < at);
        if checkpoint_due
            && !backing_off
            && let Some(editor) = entry.last_editor.clone()
        {
            let trace = TraceId::system();
            match self.checkpoint(&mut entry, &editor, &trace).await {
                Ok(_) => {
                    entry.checkpoint_failures = 0;
                    entry.checkpoint_retry_at = None;
                },
                Err(error) => {
                    entry.checkpoint_failures = entry.checkpoint_failures.saturating_add(1);
                    let wait = self
                        .tuning
                        .checkpoint_after
                        .saturating_mul(1 << entry.checkpoint_failures.min(6));
                    entry.checkpoint_retry_at = Some(now + wait);
                    if entry.checkpoint_due_from.is_none() {
                        entry.checkpoint_due_from = Some(now);
                    }
                    tracing::warn!(
                        session_id = %session,
                        error = %error,
                        failures = entry.checkpoint_failures,
                        "Edit session checkpoint failed; backing off"
                    );
                },
            }
        }
        entry.unsnapshotted == 0
            && entry.checkpoint_due_from.is_none()
            && now.duration_since(entry.touched) >= self.tuning.evict_after
    }
}
