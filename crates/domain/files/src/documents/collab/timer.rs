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
        if snapshot_due {
            if let Err(error) = self.snapshot(&mut entry).await {
                tracing::warn!(session_id = %session, error = %error, "Edit session snapshot failed");
            }
        }
        let checkpoint_due = entry
            .checkpoint_due_from
            .is_some_and(|from| now.duration_since(from) >= self.tuning.checkpoint_after);
        if checkpoint_due {
            if let Some(editor) = entry.last_editor.clone() {
                let trace = TraceId::system();
                if let Err(error) = self.checkpoint(&mut entry, &editor, &trace).await {
                    tracing::warn!(session_id = %session, error = %error, "Edit session checkpoint failed");
                }
            }
        }
        entry.unsnapshotted == 0
            && entry.checkpoint_due_from.is_none()
            && now.duration_since(entry.touched) >= self.tuning.evict_after
    }
}
